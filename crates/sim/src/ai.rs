//! The computer opponent (PLAN.md section 7, M6; docs/DECISIONS.md D42 and D43).
//!
//! A computer player is a controller in [`AiPlayers`], given by match setup
//! (`crate::match_setup::setup_match`). In the `Ai` stage of every tick, each controller whose turn
//! it is (every `think_interval` ticks of its difficulty) looks at what its player may know and
//! pushes commands into [`CommandQueue`], which the `Commands` stage of the next tick validates and
//! applies like any other player's (invariant I5). It changes no other simulation state; its own
//! state (memory, attack wave, sweep) is hashed (invariant I3).
//!
//! What a controller knows (docs/DECISIONS.md D43): its player's own units, buildings, credits,
//! power, and queues; the other players' units and buildings only while its player's vision shows
//! them (`Vision::sees_unit`, `Vision::sees_footprint`), and a memory of the buildings it has seen,
//! forgotten when their cells are seen empty; the current ore of cells only while they are
//! visible; the occupants and buildings of cells only while they are visible; and, as public match
//! setup, the static terrain of the map (passable and buildable as the map file defines them) and
//! the start positions of the match.
//!
//! What it does at each turn, in this order (each a bounded number of commands):
//!
//! 0. (RA1) while it owns the base unit of a side (the MCV it starts with), deploys it where it
//!    stands and does nothing else in that turn, so that none of its units walks into the
//!    yard's footprint before the command is applied; in its first turns it waits instead
//!    until it sees the footprint ([`deploy_base_unit`]);
//! 1. places a ready building on a free spot near its base ([`find_placement`]);
//! 2. queues the next building of the build order that it does not own as often as the build
//!    order lists it (so lost buildings are rebuilt), or a power building first if the power would
//!    not suffice; with a rules file (RA2) it does so in each lane of the Construction Yard, so
//!    that a building and a defence are built side by side ([`queue_buildings`]); but when it
//!    has no armed unit, no working harvester, and too few credits for the
//!    army unit it can produce, it sells a building its army does not need instead, and queues no
//!    building until it has an armed unit again ([`broke`]); and when it can neither fight
//!    nor earn and nothing of its state has changed for [`LAST_STAND_PATIENCE`] ticks, it
//!    sells its buildings, one a turn, so that their survivors fight or the match ends
//!    ([`last_stand`], RA4);
//! 3. queues harvesters up to `harvesters_per_refinery` per refinery, and armed units of the army
//!    composition up to `max_army`, at most [`AI_UNIT_QUEUE`] items per producing building type;
//! 4. sets the rally point of its new unit producers to its gathering point, and sends idle
//!    harvesters back to work, at most once every [`HARVEST_RETRY_TICKS`];
//! 5. defends: visible enemies within `defence_radius` of its buildings are attacked by its army
//!    at home; otherwise idle units at home gather at the gathering point;
//!
//!    Its army is its armed units that are no harvesters: an armed harvester mines, and fights
//!    only as a last resort ([`View::fights`], RA1).
//! 6. attacks: from tick `first_wave`, once `wave_size` armed units are at home, or any after
//!    `wave_patience` more ticks, they join the attack wave, which attack-moves to the nearest
//!    building it remembers, else to an enemy start position it has never seen, else sweeps the
//!    map's waypoints, those seen longest ago first and among them the one nearest to an enemy
//!    start position ([`choose_target`]); a wave reduced below half the wave size
//!    falls back to the gathering point and rejoins the units at home. On large maps (more than
//!    [`ARMY_SCALE_SPAN`] cells wide and high together) a new wave first passes a staging waypoint
//!    beside the direct route, on alternating sides ([`staging_point`]), so that the waves of two
//!    players do not always meet head-on halfway.

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::query::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Query, Res, ResMut};

use crate::ai_defs::{AiDefinitions, AiPlan, Difficulty};
use crate::arsenal::{Domain, Role};
use crate::building::{Building, Footprint, Producer};
use crate::combat::{Health, Sight, Weapon};
use crate::command::{Command, CommandQueue, IssuedCommand, PlayerId};
use crate::definitions::{Definitions, UnitKind};
use crate::economy::{Lane, PlayerState, Players, QueueKey};
use crate::economy_commands::{
    check_buildable, check_placement_site, extends_base, placement_reach, produces,
};
use crate::economy_defs::{BuildingKind, ProductionItem};
use crate::harvest::{HarvestState, Harvester};
use crate::hash::Fnv1a64;
use crate::id::SimId;
use crate::map::{CellPos, Grid};
use crate::match_setup::MatchState;
use crate::pathfinding::octile_distance;
use crate::schedule::SimTick;
use crate::unit::{Mobile, Orders, Owner, Unit};
use crate::visibility::Vision;

mod strategy;

/// Items a computer player keeps queued per producing building type of units.
pub const AI_UNIT_QUEUE: usize = 2;

/// Distance in cells from its start position toward the map's centre of a computer player's
/// gathering point.
pub const GATHER_DISTANCE: i32 = 6;

/// Idle units at home farther than this (Chebyshev distance in cells) from the gathering point
/// are sent back to it.
pub const GATHER_RADIUS: u32 = 4;

/// Ticks after a `Harvest` order for idle harvesters before the next one: an idle harvester found
/// no ore it can reach or was blocked, which a new order every turn would not change, and every
/// order makes the simulation search the map for ore.
pub const HARVEST_RETRY_TICKS: u64 = 200;

/// Ticks for which a computer player that can neither fight nor earn must have stood still (the
/// same credits, units, buildings and progress of its queues) before it makes its last stand
/// and sells its buildings ([`last_stand`]; RA4, docs/DECISIONS.md R53).
pub const LAST_STAND_PATIENCE: u64 = 1200;

/// The length in ticks of the turn of each player id in which that player may sell in its last
/// stand: two players never sell their last building in the same tick, which would end a
/// match without a winner.
pub const LAST_STAND_TURN: u64 = 256;

/// Times an attack wave may come to rest at a remembered building that is still standing before
/// the building is no longer chosen as a target (it cannot be reached).
pub const MAX_TARGET_FAILURES: u8 = 3;

/// Smallest Chebyshev distance in cells between a building a computer player places and its own
/// buildings: one free cell between them, so that its units can always walk around them.
const BUILDING_GAP: u32 = 2;

/// Smallest Chebyshev distance between a building a computer player places and its own buildings
/// that produce units: a unit that leaves such a building in the tick between the decision and the
/// command's validation, and steps on at once, cannot reach the new footprint (D43).
const PRODUCER_GAP: u32 = 3;

const MAX_AI_COMMANDS: usize = 32;

/// A building another player owns that a computer player has seen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KnownBuilding {
    pub owner: PlayerId,
    pub footprint: Footprint,
    pub kind: BuildingKind,
}

/// A cell an attack wave sweeps, and when the computer player last saw it (the tick index plus
/// one; 0 if never). `priority` marks an enemy start position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Waypoint {
    pub cell: CellPos,
    pub seen: u64,
    pub priority: bool,
}

/// The attack wave's target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaveTarget {
    /// A remembered building.
    Building(SimId),
    /// The waypoint at this index.
    Waypoint(usize),
}

/// One computer player's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiController {
    /// The entries of the AI file its faction offers (M7, docs/DECISIONS.md D49).
    plan: AiPlan,
    difficulty: Difficulty,
    start: CellPos,
    gather: CellPos,
    waypoints: Vec<Waypoint>,
    known: BTreeMap<SimId, KnownBuilding>,
    /// Huts and collapsed decks learned through current sight; retained while fog returns.
    repair_huts: BTreeMap<SimId, (SimId, CellPos)>,
    broken_bridges: BTreeSet<SimId>,
    wave: BTreeSet<SimId>,
    target: Option<WaveTarget>,
    failures: BTreeMap<SimId, u8>,
    next_army: usize,
    /// Country composition progress is independent for each real production FIFO.
    army_cursors: BTreeMap<QueueKey, usize>,
    /// The tick from which the units at home attack even if fewer than the wave size.
    patience_until: u64,
    /// The tick from which idle harvesters may be ordered to work again.
    next_harvest: u64,
    /// Whether the wave was sent smaller than the wave size (after the patience ran out), so that
    /// it does not fall back when it shrinks.
    fall_back_exempt: bool,
    /// Whether it sold a building to fund an army unit and has no armed unit yet ([`broke`]).
    liquidating: bool,
    issued: u64,
    /// Whether waves flank through a staging point (large maps only).
    flank: bool,
    /// The staging point the current wave moves through before its target, and the tick by
    /// which it gives up on reaching it.
    staging: Option<(CellPos, u64)>,
    /// Waves sent so far; its parity picks the side of the next staging point.
    waves_sent: u64,
    /// While the player can neither fight nor earn: what it had when that last changed (its
    /// credits, units, buildings and the progress of its queues, folded into one number) and
    /// the tick of that change ([`last_stand`], RA4).
    stalled: (u64, u64),
    /// Whether it is selling its buildings in its last stand.
    last_stand: bool,
    next_wave: u64,
    seen_air: bool,
    unseen_loss: bool,
    observed_units: BTreeMap<SimId, CellPos>,
    reserved: BTreeSet<SimId>,
    naval_goal: Option<CellPos>,
    naval_site: Option<CellPos>,
    next_naval_scan: u64,
    /// Progress of each observed AA clearance job; reinforcements do not renew a failed hold.
    air_screens: BTreeMap<SimId, AirScreenProgress>,
    /// Observed attack attempts; replacement IDs do not erase a repaired job's history.
    building_jobs: BTreeMap<SimId, BuildingJobProgress>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BuildingJobProgress {
    hp: i32,
    failures: u8,
    /// Outstanding owned attackers and their ammunition at admission (zero for non-aircraft).
    actors: BTreeMap<SimId, u32>,
    founders: BTreeSet<SimId>,
    approaching: bool,
}

#[cfg(test)]
#[path = "ai/ra8_attrition_tests.rs"]
mod ra8_attrition_tests;

#[cfg(test)]
#[path = "ai/ra8_recovery_tests.rs"]
mod ra8_recovery_tests;

#[cfg(test)]
#[path = "ai/ra8_composition_tests.rs"]
mod ra8_composition_tests;

#[derive(Clone, Debug, PartialEq, Eq)]
struct AirScreenProgress {
    remaining: u32,
    distance: u32,
    hp: i32,
    last_progress: u64,
}

impl AirScreenProgress {
    fn advance(&mut self, remaining: u32, distance: u32, hp: i32, tick: u64) {
        if remaining < self.remaining || distance < self.distance || hp < self.hp {
            self.last_progress = tick;
        }
        self.remaining = self.remaining.min(remaining);
        self.distance = self.distance.min(distance);
        self.hp = self.hp.min(hp);
    }
}

/// Width + height of the largest map on which a difficulty's army applies unscaled.
pub const ARMY_SCALE_SPAN: u32 = 128;

/// `difficulty` for the map of `grid`: on maps wider and higher than [`ARMY_SCALE_SPAN`] cells
/// together, the army and its waves grow with the distance they cover, up to twice their size
/// (docs/DECISIONS.md, "Large maps with environments"). Waves of a small-map size meet in the
/// middle of a large map and fall back before either reaches the other base; bigger armies break
/// through. Their assembly patience grows by the same factor: doubling the desired wave
/// without giving production more time still sends the original small waves when the timer
/// expires (RA5, with slower harvesting and regrowing ore). Integer arithmetic throughout.
pub fn scaled_for_map(mut difficulty: Difficulty, grid: &Grid) -> Difficulty {
    let span = grid.width().saturating_add(grid.height());
    if span > ARMY_SCALE_SPAN {
        let percent = (span.saturating_mul(100) / ARMY_SCALE_SPAN).min(200);
        let scale = |value: u32| value.saturating_mul(percent) / 100;
        difficulty.max_army = scale(difficulty.max_army).min(crate::ai_defs::MAX_AI_ARMY);
        difficulty.wave_size = scale(difficulty.wave_size).min(difficulty.max_army);
        difficulty.wave_patience = scale(difficulty.wave_patience);
    }
    difficulty
}

impl AiController {
    /// A controller following `plan` (the AI file's plan for its faction, `AiDefinitions::plan_for`)
    /// with `difficulty` (one of the AI file's difficulties) for a player starting at `start`,
    /// whose enemies start at `enemy_starts`; match setup has checked that every start position is
    /// a cell of the map with passable terrain. The gathering point and the waypoints come from the
    /// static terrain of `grid` (docs/DECISIONS.md D43), so they lie on the map. `None` if no
    /// gathering point exists ([`gathering_point`]), which match setup reports as an error instead
    /// of falling back to a cell its commands could not use (M6 review 1 P1-1).
    ///
    /// `yard` is the footprint of the Construction Yard the player's base unit will deploy into
    /// (RA1), if it starts with one: the gathering point keeps clear of it, as it does of the
    /// buildings a player starts with.
    pub(crate) fn new(
        grid: &Grid,
        definitions: &Definitions,
        plan: AiPlan,
        difficulty: Difficulty,
        start: CellPos,
        enemy_starts: Vec<CellPos>,
        yard: Option<Footprint>,
    ) -> Option<Self> {
        let mut difficulty = scaled_for_map(difficulty, grid);
        if plan.rules_difficulty
            && let Some(rule) = definitions
                .rules()
                .and_then(|r| r.difficulties.iter().find(|r| r.id == difficulty.id))
        {
            difficulty.first_wave = rule.team_delay.unsigned_abs();
        }
        let (width, height) = (grid_side(grid.width()), grid_side(grid.height()));
        // In `i64`, so that no start position overflows (invariant I12).
        let centre = (i64::from(width / 2), i64::from(height / 2));
        let (dx, dy) = (centre.0 - i64::from(start.x), centre.1 - i64::from(start.y));
        let length = dx.abs().max(dy.abs());
        let reach = i64::from(GATHER_DISTANCE);
        let (x, y) = if length <= reach {
            centre
        } else {
            (
                i64::from(start.x) + dx * reach / length,
                i64::from(start.y) + dy * reach / length,
            )
        };
        // Between the start and the centre of the map, so it fits in `i32`.
        let toward = CellPos::new(
            i32::try_from(x).unwrap_or(start.x),
            i32::try_from(y).unwrap_or(start.y),
        );
        let gather = gathering_point(grid, start, toward, yard)?;
        // Waypoints in blocks as wide as the shortest sight of the army, so that a unit near a
        // block's waypoint sees the whole block.
        let spacing = plan
            .army
            .iter()
            .filter_map(|&kind| definitions.unit_combat(kind))
            .map(|terms| terms.sight)
            .min()
            .unwrap_or(1)
            .clamp(1, 16) as i32;
        let mut waypoints: Vec<Waypoint> = enemy_starts
            .iter()
            .map(|&cell| Waypoint {
                cell,
                seen: 0,
                priority: true,
            })
            .collect();
        let mut y0 = 0;
        while y0 < height {
            let mut x0 = 0;
            while x0 < width {
                let x1 = (x0 + spacing - 1).min(width - 1);
                let y1 = (y0 + spacing - 1).min(height - 1);
                let centre = CellPos::new((x0 + x1) / 2, (y0 + y1) / 2);
                let cell = (y0..=y1)
                    .flat_map(|y| (x0..=x1).map(move |x| CellPos::new(x, y)))
                    .filter(|&cell| grid.cell(cell).is_some_and(|terrain| terrain.passable))
                    .min_by_key(|&cell| (chebyshev(cell, centre), cell.y, cell.x));
                if let Some(cell) = cell {
                    waypoints.push(Waypoint {
                        cell,
                        seen: 0,
                        priority: false,
                    });
                }
                x0 += spacing;
            }
            y0 += spacing;
        }
        Some(Self {
            plan,
            start,
            gather,
            waypoints,
            known: BTreeMap::new(),
            repair_huts: BTreeMap::new(),
            broken_bridges: BTreeSet::new(),
            wave: BTreeSet::new(),
            target: None,
            failures: BTreeMap::new(),
            next_army: 0,
            army_cursors: BTreeMap::new(),
            patience_until: u64::from(difficulty.first_wave)
                .saturating_add(u64::from(difficulty.wave_patience)),
            next_harvest: 0,
            fall_back_exempt: false,
            liquidating: false,
            issued: 0,
            flank: grid.width().saturating_add(grid.height()) > ARMY_SCALE_SPAN,
            staging: None,
            waves_sent: 0,
            stalled: (0, 0),
            last_stand: false,
            next_wave: 0,
            seen_air: false,
            unseen_loss: false,
            observed_units: BTreeMap::new(),
            reserved: BTreeSet::new(),
            naval_goal: None,
            naval_site: None,
            next_naval_scan: 0,
            air_screens: BTreeMap::new(),
            building_jobs: BTreeMap::new(),
            difficulty,
        })
    }

    /// The plan it follows: the entries of the AI file its faction offers.
    pub fn plan(&self) -> &AiPlan {
        &self.plan
    }

    pub fn difficulty(&self) -> &Difficulty {
        &self.difficulty
    }

    /// The player's start position.
    pub fn start(&self) -> CellPos {
        self.start
    }

    /// Where idle units at home gather and produced units go.
    pub fn gather(&self) -> CellPos {
        self.gather
    }

    pub fn waypoints(&self) -> &[Waypoint] {
        &self.waypoints
    }

    /// The other players' buildings it remembers.
    pub fn known_buildings(&self) -> &BTreeMap<SimId, KnownBuilding> {
        &self.known
    }

    /// The units of its attack wave.
    pub fn wave(&self) -> &BTreeSet<SimId> {
        &self.wave
    }

    pub fn target(&self) -> Option<WaveTarget> {
        self.target
    }

    pub fn observed_aircraft(&self) -> bool {
        self.seen_air
    }
    pub fn needs_detector(&self) -> bool {
        self.unseen_loss
    }
    pub fn naval_target(&self) -> Option<CellPos> {
        self.naval_goal
    }

    /// Commands it has pushed into the command queue so far.
    pub fn commands_issued(&self) -> u64 {
        self.issued
    }

    fn hash_into(&self, hasher: &mut Fnv1a64) {
        self.plan.hash_into(hasher);
        self.difficulty.hash_into(hasher);
        write_cell(hasher, self.start);
        write_cell(hasher, self.gather);
        hasher.write_u64(self.waypoints.len() as u64);
        for waypoint in &self.waypoints {
            write_cell(hasher, waypoint.cell);
            hasher.write_u64(waypoint.seen);
            hasher.write(&[u8::from(waypoint.priority)]);
        }
        hasher.write_u64(self.known.len() as u64);
        for (id, known) in &self.known {
            hasher.write_u32(id.0);
            hasher.write(&[known.owner.0]);
            hasher.write(&known.kind.0.to_le_bytes());
            write_cell(hasher, known.footprint.origin);
            hasher.write(&[known.footprint.width, known.footprint.height]);
        }
        if !self.repair_huts.is_empty() || !self.broken_bridges.is_empty() {
            hasher.write(b"ai_bridge_memory");
            hasher.write_u64(self.repair_huts.len() as u64);
            for (hut, (bridge, cell)) in &self.repair_huts {
                hasher.write_u32(hut.0);
                hasher.write_u32(bridge.0);
                write_cell(hasher, *cell);
            }
            hasher.write_u64(self.broken_bridges.len() as u64);
            for id in &self.broken_bridges {
                hasher.write_u32(id.0);
            }
        }
        hasher.write_u64(self.wave.len() as u64);
        for id in &self.wave {
            hasher.write_u32(id.0);
        }
        match self.target {
            None => hasher.write(&[0]),
            Some(WaveTarget::Building(id)) => {
                hasher.write(&[1]);
                hasher.write_u32(id.0);
            }
            Some(WaveTarget::Waypoint(index)) => {
                hasher.write(&[2]);
                hasher.write_u64(index as u64);
            }
        }
        hasher.write_u64(self.failures.len() as u64);
        for (id, count) in &self.failures {
            hasher.write_u32(id.0);
            hasher.write(&[*count]);
        }
        hasher.write_u64(self.next_army as u64);
        if !self.army_cursors.is_empty() {
            hasher.write(b"ai_army_queue_cursors");
            hasher.write_u64(self.army_cursors.len() as u64);
            for (key, index) in &self.army_cursors {
                hasher.write(&key.producer.0.to_le_bytes());
                hasher.write(&[key.lane as u8]);
                hasher.write_u64(*index as u64);
            }
        }
        hasher.write_u64(self.patience_until);
        hasher.write_u64(self.next_harvest);
        hasher.write(&[u8::from(self.fall_back_exempt), u8::from(self.liquidating)]);
        hasher.write_u64(self.issued);
        hasher.write(&[u8::from(self.flank)]);
        match self.staging {
            None => hasher.write(&[0]),
            Some((cell, until)) => {
                hasher.write(&[1]);
                hasher.write_u32(cell.x as u32);
                hasher.write_u32(cell.y as u32);
                hasher.write_u64(until);
            }
        }
        hasher.write_u64(self.waves_sent);
        hasher.write_u64(self.stalled.0);
        hasher.write_u64(self.stalled.1);
        hasher.write(&[u8::from(self.last_stand)]);
        hasher.write(&[u8::from(self.seen_air), u8::from(self.unseen_loss)]);
        hasher.write_u64(self.next_wave);
        hasher.write_u64(self.next_naval_scan);
        hasher.write_u64(self.air_screens.len() as u64);
        for (id, progress) in &self.air_screens {
            hasher.write_u32(id.0);
            hasher.write_u32(progress.remaining);
            hasher.write_u32(progress.distance);
            hasher.write_u32(progress.hp as u32);
            hasher.write_u64(progress.last_progress);
        }
        hasher.write_u64(self.building_jobs.len() as u64);
        for (id, progress) in &self.building_jobs {
            hasher.write_u32(id.0);
            hasher.write(&progress.hp.to_le_bytes());
            hasher.write(&[progress.failures]);
            hasher.write_u64(progress.actors.len() as u64);
            for (actor, ammo) in &progress.actors {
                hasher.write_u32(actor.0);
                hasher.write_u32(*ammo);
            }
            hasher.write_u64(progress.founders.len() as u64);
            for actor in &progress.founders {
                hasher.write_u32(actor.0);
            }
            hasher.write(&[u8::from(progress.approaching)]);
        }
        hasher.write_u64(self.observed_units.len() as u64);
        for (id, cell) in &self.observed_units {
            hasher.write_u32(id.0);
            write_cell(hasher, *cell);
        }
        hasher.write_u64(self.reserved.len() as u64);
        for id in &self.reserved {
            hasher.write_u32(id.0);
        }
        match self.naval_goal {
            None => hasher.write(&[0]),
            Some(cell) => {
                hasher.write(&[1]);
                write_cell(hasher, cell);
            }
        }
        match self.naval_site {
            None => hasher.write(&[0]),
            Some(cell) => {
                hasher.write(&[1]);
                write_cell(hasher, cell);
            }
        }
    }
}

/// The gathering point of a player starting at `start` (a cell of the map with passable terrain):
/// of the cells reachable from `start` over passable terrain (four-neighbour connectivity, which
/// for the eight-neighbour steps of equation 5, with no diagonal step between two blocked
/// orthogonal neighbours, connects the same cells), those that no building covers, the one nearest
/// to `toward` (octile distance, then row, then column). `None` if there is none, for example a
/// start walled in by terrain under its own buildings. Its units can walk there unless buildings
/// placed later block the way. The cells of `reserved` (the yard a base unit will deploy into)
/// count as covered by a building.
fn gathering_point(
    grid: &Grid,
    start: CellPos,
    toward: CellPos,
    reserved: Option<Footprint>,
) -> Option<CellPos> {
    let width = usize::try_from(grid.width()).ok()?;
    let index = |cell: CellPos| -> Option<usize> {
        grid.contains(cell).then(|| {
            usize::try_from(cell.y)
                .unwrap_or(0)
                .saturating_mul(width)
                .saturating_add(usize::try_from(cell.x).unwrap_or(0))
        })
    };
    let terrain = |cell: CellPos| grid.cell(cell).is_some_and(|terrain| terrain.passable);
    if !terrain(start) {
        return None;
    }
    let mut reached = vec![false; width.saturating_mul(usize::try_from(grid.height()).ok()?)];
    let mut stack = vec![start];
    reached[index(start)?] = true;
    let mut best: Option<(u32, i32, i32)> = None;
    while let Some(cell) = stack.pop() {
        if grid.building(cell).is_none() && !reserved.is_some_and(|yard| yard.contains(cell)) {
            let key = (octile_distance(cell, toward), cell.y, cell.x);
            if best.is_none_or(|best| key < best) {
                best = Some(key);
            }
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let next = CellPos::new(cell.x.saturating_add(dx), cell.y.saturating_add(dy));
            if let Some(next_index) = index(next)
                && terrain(next)
                && !std::mem::replace(&mut reached[next_index], true)
            {
                stack.push(next);
            }
        }
    }
    best.map(|(_, y, x)| CellPos::new(x, y))
}

fn grid_side(cells: u32) -> i32 {
    i32::try_from(cells).unwrap_or(i32::MAX)
}

fn write_cell(hasher: &mut Fnv1a64, cell: CellPos) {
    hasher.write(&cell.x.to_le_bytes());
    hasher.write(&cell.y.to_le_bytes());
}

fn chebyshev(a: CellPos, b: CellPos) -> u32 {
    let dx = (i64::from(a.x) - i64::from(b.x)).unsigned_abs();
    let dy = (i64::from(a.y) - i64::from(b.y)).unsigned_abs();
    u32::try_from(dx.max(dy)).unwrap_or(u32::MAX)
}

fn cell_footprint(cell: CellPos) -> Footprint {
    Footprint {
        origin: cell,
        width: 1,
        height: 1,
    }
}

/// The computer players of the match: the AI file and one controller per computer player.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct AiPlayers {
    /// `None` until match setup installs the AI file.
    definitions: Option<AiDefinitions>,
    controllers: BTreeMap<PlayerId, AiController>,
}

impl AiPlayers {
    pub(crate) fn thinks_at(&self, tick: u64, state: &MatchState) -> bool {
        state.outcome().is_none()
            && self.controllers.iter().any(|(player, controller)| {
                state.defeated(*player).is_none() && due(*player, controller, tick)
            })
    }

    pub(crate) fn new(
        definitions: AiDefinitions,
        controllers: BTreeMap<PlayerId, AiController>,
    ) -> Self {
        Self {
            definitions: Some(definitions),
            controllers,
        }
    }

    /// The controller of computer player `player`.
    pub fn controller(&self, player: PlayerId) -> Option<&AiController> {
        self.controllers.get(&player)
    }

    /// The computer players in ascending id order.
    pub fn players(&self) -> impl Iterator<Item = PlayerId> + '_ {
        self.controllers.keys().copied()
    }

    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        match &self.definitions {
            None => hasher.write(&[0]),
            Some(definitions) => {
                hasher.write(&[1]);
                definitions.hash_into(hasher);
            }
        }
        hasher.write_u64(self.controllers.len() as u64);
        for (player, controller) in &self.controllers {
            hasher.write(&[player.0]);
            controller.hash_into(hasher);
        }
    }
}

fn due(player: PlayerId, controller: &AiController, tick: u64) -> bool {
    let interval = u64::from(controller.difficulty.think_interval.max(1));
    tick % interval == u64::from(player.0) % interval
        || (controller.last_stand
            && tick.is_multiple_of(LAST_STAND_TURN)
            && (tick / LAST_STAND_TURN) % 16 == u64::from(player.0 % 16))
}

// --- What the controllers see ---------------------------------------------------------------

/// A unit, as read at the start of the `Ai` stage.
struct UnitSnap {
    hp: i32,
    id: SimId,
    owner: PlayerId,
    kind: UnitKind,
    mobile: Mobile,
    has_order: bool,
    harvest: Option<HarvestState>,
    /// Whether it has a weapon against targets on the ground. Whether it belongs to the army is
    /// [`View::fights`].
    armed: bool,
    attacking: bool,
    /// Existing combat intent, derived from the already hashed Weapon state.
    attack_order: Option<crate::AttackOrder>,
    combat_target: Option<SimId>,
    /// Actual movement progress from the already hashed current order.
    move_progress: Option<(CellPos, u32)>,
    sight: Option<u32>,
}

impl UnitSnap {
    /// No move order and no combat order: it stands and only fires at what comes into range.
    fn idle(&self) -> bool {
        !self.has_order && !self.attacking
    }
}

/// A building, as read at the start of the `Ai` stage.
struct BuildingSnap {
    economy: bool,
    hp: i32,
    id: SimId,
    owner: PlayerId,
    kind: BuildingKind,
    footprint: Footprint,
    /// `None` for a building that produces no units, else its rally point.
    producer: Option<Option<CellPos>>,
    sight: Option<u32>,
    pending: Vec<UnitKind>,
}

/// What one computer player may know (docs/DECISIONS.md D43). [`think`] reads nothing else.
struct View<'a> {
    known_occupants: BTreeSet<SimId>,
    owned_units: Vec<&'a UnitSnap>,
    frozen_army: BTreeSet<SimId>,
    recoverable_garrisons: Vec<SimId>,
    recoverable_service: Vec<&'a UnitSnap>,
    seen_tech: Vec<&'a BuildingSnap>,
    living_owned_ids: BTreeSet<SimId>,
    player: PlayerId,
    tick: u64,
    state: &'a PlayerState,
    vision: &'a Vision,
    grid: &'a Grid,
    definitions: &'a Definitions,
    /// The computer player's plan (named `ai` as the AI file it is taken from).
    ai: &'a AiPlan,
    /// Whether the match has superweapons switched on (`MatchRules`, public match setup).
    superweapons: bool,
    /// The types of the player's buildings, one entry per building.
    own_kinds: Vec<BuildingKind>,
    /// Whether the player has no armed unit other than harvesters ([`View::fights`]).
    last_resort: bool,
    own_units: Vec<&'a UnitSnap>,
    own_buildings: Vec<&'a BuildingSnap>,
    /// Other players' units its player sees.
    enemy_units: Vec<&'a UnitSnap>,
    /// Other players' buildings its player sees.
    enemy_buildings: Vec<&'a BuildingSnap>,
}

impl<'a> View<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        player: PlayerId,
        tick: u64,
        players: &Players,
        state: &'a PlayerState,
        grid: &'a Grid,
        definitions: &'a Definitions,
        ai: &'a AiPlan,
        superweapons: bool,
        units: &'a [UnitSnap],
        buildings: &'a [BuildingSnap],
        special: &crate::SpecialState,
    ) -> Self {
        let vision = state.vision();
        // Allies (a team of computer players against the human) are never targets.
        let hostile = |owner: PlayerId| players.hostile(player, owner);
        // RA4: a missile or an aircraft that another unit launched is no unit of the player's
        // to command or to count on (it is gone when it has done its work), and a unit that is
        // hidden from the player (a submerged, disguised or mirage unit) is not seen.
        let hidden = &special.hidden;
        let service_candidate = |unit: &&UnitSnap| {
            unit.owner == player
                && unit.hp > 0
                && !unit.has_order
                && !unit.attacking
                && available_for_army(special, unit.id)
                && !special.parasites.contains_key(&unit.id)
                && !definitions.on_foot(unit.kind)
                && definitions.domain(unit.kind) != Domain::Air
                && special.docked.get(&unit.id).is_some_and(|host| {
                    buildings.iter().any(|b| {
                        b.id == *host
                            && b.economy
                            && b.owner == player
                            && b.hp > 0
                            && b.footprint.distance_to_cell(unit.mobile.cell) <= 1
                            && definitions
                                .metadata(ProductionItem::Building(b.kind))
                                .is_some_and(|m| m.unit_repair)
                    })
                })
        };
        Self {
            owned_units: units.iter().filter(|u| u.owner == player).collect(),
            frozen_army: units
                .iter()
                .filter(|u| {
                    u.owner == player
                        && u.armed
                        && special.disabled.contains_key(&u.id)
                        && !special.docked.contains_key(&u.id)
                        && !special.parasites.contains_key(&u.id)
                        && available_when_enabled(special, u.id)
                })
                .map(|u| u.id)
                .collect(),
            known_occupants: units
                .iter()
                .filter(|unit| {
                    unit.owner == player
                        || (vision.sees_unit(&unit.mobile) && !hidden.contains(&(player, unit.id)))
                })
                .map(|unit| unit.id)
                .collect(),
            recoverable_service: units.iter().filter(service_candidate).collect(),
            recoverable_garrisons: special
                .garrisons
                .iter()
                .filter(|(id, g)| {
                    g.owner == player
                        && !g.evicting
                        && special.cargo.get(id).is_some_and(|cargo| !cargo.is_empty())
                })
                .map(|(&id, _)| id)
                .collect(),
            living_owned_ids: units
                .iter()
                .filter(|u| {
                    u.owner == player
                        && !special.spawned.contains_key(&u.id)
                        && !special.strategic.flights.contains_key(&u.id)
                        && definitions
                            .metadata(ProductionItem::Unit(u.kind))
                            .is_none_or(|m| {
                                !matches!(m.role, Role::Engineer | Role::Spy | Role::Mcv)
                            })
                })
                .map(|u| u.id)
                .chain(
                    special
                        .cargo
                        .iter()
                        .filter(|(host, _)| {
                            units.iter().any(|u| u.id == **host && u.owner == player)
                                || buildings
                                    .iter()
                                    .any(|b| b.id == **host && b.owner == player)
                                || special
                                    .garrisons
                                    .get(host)
                                    .is_some_and(|g| g.owner == player)
                        })
                        .flat_map(|(_, cargo)| {
                            cargo
                                .iter()
                                .filter(|passenger| {
                                    definitions
                                        .metadata(ProductionItem::Unit(passenger.kind))
                                        .is_none_or(|meta| {
                                            !matches!(
                                                meta.role,
                                                Role::Engineer | Role::Spy | Role::Mcv
                                            )
                                        })
                                })
                                .map(|p| p.id)
                        }),
                )
                .chain(
                    special
                        .parasites
                        .values()
                        .filter(|p| p.owner == player)
                        .map(|p| p.passenger.id),
                )
                .collect(),
            seen_tech: buildings
                .iter()
                .filter(|b| {
                    (b.owner == crate::NEUTRAL || hostile(b.owner))
                        && vision.sees_footprint(b.footprint)
                        && definitions
                            .metadata(ProductionItem::Building(b.kind))
                            .is_some_and(|m| {
                                m.needs_engineer && m.capturable && !m.bridge_repair_hut
                            })
                })
                .collect(),
            player,
            tick,
            state,
            vision,
            grid,
            definitions,
            ai,
            superweapons,
            last_resort: !units.iter().any(|unit| {
                unit.owner == player
                    && unit.armed
                    && unit.harvest.is_none()
                    && (army_commandable(special, unit.id) || service_candidate(&unit))
            }),
            own_kinds: buildings
                .iter()
                .filter(|building| building.owner == player && building.economy)
                .map(|building| building.kind)
                .collect(),
            own_units: units
                .iter()
                .filter(|unit| unit.owner == player && army_commandable(special, unit.id))
                .collect(),
            own_buildings: buildings
                .iter()
                .filter(|building| building.owner == player && building.economy)
                .collect(),
            enemy_units: units
                .iter()
                .filter(|unit| {
                    hostile(unit.owner)
                        && vision.sees_unit(&unit.mobile)
                        && !hidden.contains(&(player, unit.id))
                })
                .collect(),
            enemy_buildings: buildings
                .iter()
                .filter(|building| {
                    hostile(building.owner) && vision.sees_footprint(building.footprint)
                })
                .collect(),
        }
    }

    fn owned(&self, kind: BuildingKind) -> usize {
        self.own_buildings
            .iter()
            .filter(|building| building.kind == kind)
            .count()
    }

    /// Whether `unit` (one of the player's) belongs to its army: it has a weapon and is no
    /// harvester. An armed harvester (the Soviet War Miner) mines: drafted into the army it
    /// would be called to the gathering point and into the waves, and its player would earn
    /// nothing. Only as a last resort does it fight: while the player has no other armed unit
    /// and the harvester's cycle has stopped (no ore or no refinery it can reach), so that a
    /// player left with nothing else still attacks (RA1, docs/DECISIONS.md R14).
    fn fights(&self, unit: &UnitSnap) -> bool {
        unit.armed
            && match unit.harvest {
                None => true,
                Some(state) => self.last_resort && state == HarvestState::Idle,
            }
    }

    /// Whether the player could produce `item` now, as far as `QueueProduction` checks it
    /// before the length of the queue (`check_buildable`).
    fn buildable(&self, item: ProductionItem) -> bool {
        check_buildable(
            self.definitions,
            self.state,
            self.superweapons,
            &self.own_kinds,
            item,
        )
        .is_ok()
    }

    /// Whether `QueueProduction` of `item` would pass its validation now: the player may build
    /// it, and the queue is not full.
    fn queueable(&self, item: ProductionItem) -> bool {
        self.buildable(item) && self.queue_len(item) < self.definitions.queue_capacity(item)
    }

    /// The building type and lane under which the queue of `item` is kept.
    fn queue_key(&self, item: ProductionItem) -> Option<QueueKey> {
        self.definitions.queue_of(item)
    }

    fn queue_len(&self, item: ProductionItem) -> usize {
        self.queue_key(item)
            .map_or(usize::MAX, |key| self.state.lane(key).len())
    }

    /// Items of `item` in the player's queues.
    fn queued(&self, item: ProductionItem) -> usize {
        self.state
            .queues()
            .flat_map(|(_, items)| items.iter())
            .filter(|queued| queued.item == item)
            .count()
    }

    /// Whether `cell` is certain to be visible to the player after the next `Visibility` stage,
    /// unless one of its units or buildings is destroyed first: it lies within the sight of one of
    /// its buildings, or within the sight of one of its units both from the cell the unit stands in
    /// and from the cell it steps into (in one tick a unit's own cell can only become the cell it
    /// steps into, `crate::movement`) (equation 6). A unit that teleports is not counted: a
    /// Chrono Miner with a full load is at its refinery in the next tick, and a Chrono
    /// Legionnaire with a move order at its destination (`crate::special`; RA3,
    /// docs/DECISIONS.md R44).
    fn stays_visible(&self, cell: CellPos) -> bool {
        if !self.vision.uses_fog() && self.vision.is_visible(cell) {
            return true;
        }
        let within = |footprint: Footprint, sight: u32| {
            let (x0, y0) = (i64::from(footprint.origin.x), i64::from(footprint.origin.y));
            let x1 = x0 + i64::from(footprint.width.max(1)) - 1;
            let y1 = y0 + i64::from(footprint.height.max(1)) - 1;
            let (x, y) = (i64::from(cell.x), i64::from(cell.y));
            let dx = (x0 - x).max(x - x1).max(0);
            let dy = (y0 - y).max(y - y1).max(0);
            dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy))
                <= i64::from(sight) * i64::from(sight)
        };
        self.own_buildings.iter().any(|building| {
            building
                .sight
                .is_some_and(|sight| within(building.footprint, sight))
        }) || self.own_units.iter().any(|unit| {
            let teleports = self
                .definitions
                .metadata(ProductionItem::Unit(unit.kind))
                .is_some_and(|entry| {
                    entry.teleporter || matches!(entry.role, Role::ChronoMiner | Role::Chrono)
                });
            !teleports
                && unit.sight.is_some_and(|sight| {
                    within(cell_footprint(unit.mobile.cell), sight)
                        && within(cell_footprint(unit.mobile.next_centre()), sight)
                })
        })
    }
}

// --- The system -----------------------------------------------------------------------------

type UnitQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static SimId,
        &'static Health,
        &'static Owner,
        &'static Unit,
        &'static Mobile,
        &'static Orders,
        Option<&'static Harvester>,
        Option<&'static Weapon>,
        Option<&'static Sight>,
    ),
    With<Unit>,
>;

type BuildingQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static SimId,
        &'static Health,
        &'static Owner,
        &'static Building,
        Option<&'static Producer>,
        Option<&'static Sight>,
    ),
>;

// Frozen/deployed actors cannot accept army movement orders. They return to the
// candidate set after release/undeploy, without retaining a stale wave assignment.
fn army_commandable(special: &crate::SpecialState, id: SimId) -> bool {
    !special.docked.contains_key(&id) && available_for_army(special, id)
}

fn available_for_army(special: &crate::SpecialState, id: SimId) -> bool {
    !special.disabled.contains_key(&id) && available_when_enabled(special, id)
}

fn available_when_enabled(special: &crate::SpecialState, id: SimId) -> bool {
    !special.spawned.contains_key(&id)
        && !special.strategic.flights.contains_key(&id)
        && !special.erasures.contains_key(&id)
        && !special.deployed.contains_key(&id)
        && !special.interactions.contains_key(&id)
        && !special.rearming.contains_key(&id)
}

// Utility and deploy-only weapons do not make a unit available for army orders. In
// particular, an Engineer's DefuseKit must not compete with its bridge-repair assignment.
fn army_weapon(definitions: &Definitions, kind: crate::WeaponKind) -> bool {
    definitions.weapon(kind).is_some_and(|weapon| {
        (weapon.anti_ground || weapon.naval_only)
            && weapon.damage > 0
            && !weapon.area_fire
            && definitions.warhead(weapon.warhead).is_some_and(|warhead| {
                !warhead.bomb_disarm && !warhead.electric_assault && !warhead.makes_disguise
            })
    })
}

/// The `Ai` stage: every computer player whose turn it is decides and pushes its commands.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_ai(
    tick: Res<SimTick>,
    grid: Res<Grid>,
    definitions: Res<Definitions>,
    players: Res<Players>,
    state: Res<MatchState>,
    mut ai: ResMut<AiPlayers>,
    mut queue: ResMut<CommandQueue>,
    units: UnitQuery,
    buildings: BuildingQuery,
    special: Res<crate::SpecialState>,
    bridges: Res<crate::Bridges>,
    props: Res<crate::Props>,
) {
    if ai.controllers.is_empty() || state.outcome().is_some() {
        return;
    }
    let tick = tick.0;
    let thinking: Vec<PlayerId> = ai
        .controllers
        .iter()
        .filter(|(player, controller)| {
            due(**player, controller, tick) && state.defeated(**player).is_none()
        })
        .map(|(player, _)| *player)
        .collect();
    if thinking.is_empty() {
        return;
    }
    let mut unit_snaps: Vec<UnitSnap> = units
        .iter()
        .map(
            |(id, health, owner, unit, mobile, orders, harvester, weapon, sight)| UnitSnap {
                hp: health.hit_points,
                id: *id,
                owner: owner.0,
                kind: unit.kind,
                mobile: *mobile,
                has_order: orders.current.is_some(),
                harvest: harvester.map(|harvester| harvester.state),
                // A unit whose weapons engage aircraft only wins no ground: it is no army
                // unit (RA3).
                armed: weapon.is_some_and(|weapon| {
                    [Some(weapon.kind), weapon.secondary]
                        .into_iter()
                        .flatten()
                        .any(|kind| army_weapon(&definitions, kind))
                }),
                attacking: weapon.is_some_and(|weapon| weapon.order.is_some()),
                attack_order: weapon.and_then(|weapon| weapon.order),
                combat_target: weapon.and_then(|weapon| weapon.target),
                move_progress: orders
                    .current
                    .as_ref()
                    .map(|order| (order.destination, order.best)),
                sight: sight.map(|sight| sight.0),
            },
        )
        .collect();
    unit_snaps.retain(|unit| unit.hp > 0);
    unit_snaps.sort_unstable_by_key(|unit| unit.id);
    let mut building_snaps: Vec<BuildingSnap> = buildings
        .iter()
        .map(
            |(id, health, owner, building, producer, sight)| BuildingSnap {
                economy: true,
                hp: health.hit_points,
                id: *id,
                owner: owner.0,
                kind: building.kind,
                footprint: building.footprint,
                producer: producer.map(|producer| producer.rally),
                sight: sight.map(|sight| sight.0),
                pending: building.pending_units.clone(),
            },
        )
        .collect();
    for (id, g) in &special.garrisons {
        if let Some((cell, prop)) = props.get(*id).filter(|(_, p)| p.hit_points > 0)
            && let Some(ProductionItem::Building(kind)) =
                crate::garrison::item(&definitions, prop.kind)
        {
            building_snaps.push(BuildingSnap {
                economy: false,
                hp: prop.hit_points,
                id: *id,
                owner: g.owner,
                kind,
                footprint: Footprint {
                    origin: cell,
                    width: 1,
                    height: 1,
                },
                producer: None,
                sight: definitions.building_combat(kind).map(|t| t.sight),
                pending: Vec::new(),
            });
        }
    }
    building_snaps.retain(|building| building.hp > 0);
    building_snaps.sort_unstable_by_key(|building| building.id);

    let AiPlayers {
        definitions: Some(_),
        controllers,
    } = &mut *ai
    else {
        return;
    };
    for player in thinking {
        let (Some(controller), Some(player_state)) =
            (controllers.get_mut(&player), players.get(player))
        else {
            continue;
        };
        let plan = controller.plan.clone();
        let view = View::new(
            player,
            tick,
            &players,
            player_state,
            &grid,
            &definitions,
            &plan,
            state.rules().superweapons,
            &unit_snaps,
            &building_snaps,
            &special,
        );
        let mut commands = repair_bridge(
            &view,
            controller,
            &special,
            &bridges,
            &building_snaps,
            &unit_snaps,
        );
        let bridge_job = commands.iter().find_map(|command| match command {
            Command::Interact { unit, target } => Some((*unit, *target)),
            _ => None,
        });
        strategy::observe_attack_jobs(&view, controller, &special);
        commands.extend(think(&view, controller));
        let extra = strategy::decide(&view, controller, &special, &props);
        // A unit assigned a specialist job receives only that job this turn.
        for command in &mut commands {
            if let Command::Move { units, .. }
            | Command::AttackMove { units, .. }
            | Command::Attack { units, .. } = command
            {
                units.retain(|id| !controller.reserved.contains(id));
            }
        }
        commands.retain(|c| !matches!(c, Command::Move { units, .. } | Command::AttackMove { units, .. } | Command::Attack { units, .. } if units.is_empty()));
        commands.extend(extra);
        retain_funded_unit_intents(&view, &mut commands, controller.difficulty.think_interval);
        commands.truncate(MAX_AI_COMMANDS);
        for command in &commands {
            strategy::record_attack_job(&view, controller, &special, command);
        }
        controller.issued = controller.issued.saturating_add(commands.len() as u64);
        for command in commands {
            let mut witness = crate::command::AiIntent::default();
            let actor_ids: Vec<_> = match &command {
                Command::Deploy { unit } | Command::Interact { unit, .. } => vec![*unit],
                Command::Undeploy { building }
                | Command::SellBuilding { building }
                | Command::RepairBuilding { building }
                | Command::SetRallyPoint { building, .. }
                | Command::Chronoshift { building, .. }
                | Command::Superweapon { building, .. } => vec![*building],
                _ => command.units().unwrap_or(&[]).to_vec(),
            };
            for id in actor_ids {
                if let Some(u) = view.own_units.iter().find(|u| u.id == id) {
                    witness.owned.insert(id);
                    if u.armed {
                        witness.armed.insert(id);
                    }
                } else if matches!(command, Command::Move { .. })
                    && view.recoverable_service.iter().any(|u| u.id == id)
                {
                    // A normal Move is valid for this decision-time owned service occupant.
                    // Other docked/transient actors never obtain this cancellation witness.
                    witness.owned.insert(id);
                } else if (matches!(command, Command::Deploy { .. })
                    && view.recoverable_garrisons.contains(&id))
                    || (command.units().is_none()
                        && !matches!(command, Command::Deploy { .. } | Command::Interact { .. })
                        && view.own_buildings.iter().any(|b| b.id == id))
                {
                    witness.owned.insert(id);
                }
            }
            let target = match command {
                Command::Attack { target, .. } | Command::Interact { target, .. } => Some(target),
                _ => None,
            };
            witness.target = target.filter(|id| {
                view.enemy_units.iter().any(|u| u.id == *id)
                    || view.enemy_buildings.iter().any(|b| b.id == *id)
                    || (!matches!(command, Command::Attack { .. })
                        && (view.own_buildings.iter().any(|b| b.id == *id)
                            || view.seen_tech.iter().any(|b| b.id == *id)
                            || props
                                .get(*id)
                                .is_some_and(|(cell, _)| view.vision.is_visible(cell))))
            });
            if let Some(ids) = command.units() {
                let valid = !ids.is_empty()
                    && ids.len() <= crate::MAX_UNITS_PER_COMMAND
                    && ids.iter().copied().collect::<BTreeSet<_>>().len() == ids.len()
                    && ids.iter().all(|id| witness.owned.contains(id))
                    && (!matches!(command, Command::Attack { .. })
                        || ids.iter().all(|id| witness.armed.contains(id)))
                    && (!matches!(command, Command::Harvest { .. })
                        || ids.iter().all(|id| {
                            view.own_units
                                .iter()
                                .any(|u| u.id == *id && u.harvest.is_some())
                        }))
                    && match command {
                        Command::Move { target, .. }
                        | Command::AttackMove { target, .. }
                        | Command::Harvest { target, .. } => view.grid.contains(target),
                        _ => true,
                    };
                if !valid {
                    witness = crate::command::AiIntent::default();
                }
            }
            match command {
                Command::Interact { unit, target } if bridge_job == Some((unit, target)) => {
                    queue.push_bridge_goal(player, unit, target, witness);
                }
                command => queue.push_ai(IssuedCommand { player, command }, witness),
            }
        }
    }
}

/// Learn through current sight, then approach a remembered hut with an ordinary Move.
/// Interact still requires current visibility, and unseen repairs are not disclosed.
fn repair_bridge(
    view: &View<'_>,
    controller: &mut AiController,
    special: &crate::SpecialState,
    bridges: &crate::Bridges,
    buildings: &[BuildingSnap],
    units: &[UnitSnap],
) -> Vec<Command> {
    for building in buildings
        .iter()
        .filter(|b| view.vision.sees_footprint(b.footprint))
    {
        if let Some(&bridge) = bridges.huts.get(&building.id) {
            controller
                .repair_huts
                .insert(building.id, (bridge, building.footprint.origin));
        }
    }
    for (&id, bridge) in &bridges.bridges {
        if bridge
            .cells
            .iter()
            .any(|&cell| view.vision.is_visible(cell))
        {
            if bridge.collapsed {
                controller.broken_bridges.insert(id);
            } else {
                controller.broken_bridges.remove(&id);
            }
        }
    }
    if controller.broken_bridges.is_empty() {
        return Vec::new();
    }
    let is_engineer = |kind| {
        view.definitions
            .metadata(ProductionItem::Unit(kind))
            .is_some_and(|entry| entry.role == crate::Role::Engineer)
    };
    let engineer = view
        .own_units
        .iter()
        .filter(|u| is_engineer(u.kind))
        .min_by_key(|u| u.id);
    // The Engineer, or its home gathering point before it is trained, must reach the entrance.
    // An arbitrary army member may already be stranded on the other bank.
    let from = engineer.map_or(controller.gather, |u| u.mobile.cell);
    let hut = controller
        .repair_huts
        .iter()
        .filter(|(_, (bridge, _))| controller.broken_bridges.contains(bridge))
        .filter(|(_, (bridge, _))| {
            bridges.bridges.get(bridge).is_some_and(|bridge| {
                // Current/departing holders and reserved arrivals must clear. A known ship
                // with an active order one step away can reserve the deck later this tick,
                // before the queued Interact runs. An idle adjacent ship need not block.
                // Unknown occupants never influence this decision (including hidden subs).
                !units
                    .iter()
                    .filter(|ship| view.known_occupants.contains(&ship.id))
                    .filter(|ship| view.definitions.domain(ship.kind) == Domain::Naval)
                    .any(|ship| {
                        bridge.cells.iter().any(|&cell| {
                            ship.mobile.cell == cell
                                || ship.mobile.step.is_some_and(|step| step.to == cell)
                                || ((ship.has_order || ship.attacking)
                                    && chebyshev(ship.mobile.cell, cell) <= 1)
                        })
                    })
            })
        })
        .filter_map(|(&id, &(bridge, cell))| {
            crate::pathfinding::nearest_cell(view.grid, from, |p| {
                view.grid.is_passable(p)
                    && view.grid.building(p).is_none()
                    && chebyshev(p, cell) == 1
            })
            .map(|goal| (chebyshev(from, cell), id, bridge, cell, goal))
        })
        .min_by_key(|(distance, id, _, _, _)| (*distance, *id));
    let Some((_, hut, bridge, cell, goal)) = hut else {
        return Vec::new();
    };
    if let Some(unit) = engineer {
        if special.interactions.get(&unit.id) == Some(&hut) {
            return Vec::new();
        }
        if view.vision.is_visible(cell)
            && bridges
                .bridges
                .get(&bridge)
                .is_some_and(|b| b.cells.iter().any(|&p| view.vision.is_visible(p)))
        {
            // Only a currently observed collapsed deck justifies a fresh Interact. A remembered
            // objective that another player already repaired disappears on observation above.
            return vec![Command::Interact {
                unit: unit.id,
                target: hut,
            }];
        }
        return if unit.idle() {
            vec![Command::Move {
                units: vec![unit.id],
                target: goal,
            }]
        } else {
            Vec::new()
        };
    }
    if view.definitions.units().iter().enumerate().any(|(i, _)| {
        is_engineer(crate::UnitKind(i as u16))
            && view.queued(ProductionItem::Unit(crate::UnitKind(i as u16))) > 0
    }) {
        return Vec::new();
    }
    view.definitions
        .units()
        .iter()
        .enumerate()
        .filter_map(|(i, _)| {
            let kind = crate::UnitKind(i as u16);
            let item = ProductionItem::Unit(kind);
            (is_engineer(kind) && view.queueable(item)).then_some(Command::QueueProduction { item })
        })
        .take(1)
        .collect()
}

/// One turn of a computer player: updates its memory and returns its commands.
fn think(view: &View<'_>, controller: &mut AiController) -> Vec<Command> {
    remember(view, controller);
    match deploy_base_unit(view, controller) {
        BaseUnit::Deploy(command) => return vec![command],
        BaseUnit::Wait => return Vec::new(),
        BaseUnit::None => {}
    }
    let mut commands = Vec::new();
    let mut recovering = false;
    if let Some(command) = cancel_blocked_head(view).or_else(|| cancel_starved_construction(view)) {
        commands.push(command);
    } else {
        let recovery = recover_unit_credits(view, controller);
        recovering = !recovery.is_empty();
        commands.extend(recovery);
    }
    if let Some(command) = place_ready_building(view, controller) {
        commands.push(command);
    }
    // While it sells buildings to fund an army unit, and until it has one, it does not queue
    // buildings, which would take the credits first.
    if view
        .own_units
        .iter()
        .chain(&view.recoverable_service)
        .any(|unit| view.fights(unit))
    {
        controller.liquidating = false;
    }
    if !recovering && broke(view) {
        if let Some(command) = sell_for_army(view) {
            controller.liquidating = true;
            commands.push(command);
        }
    } else if !recovering && !controller.liquidating {
        commands.extend(queue_buildings(view, controller));
    }
    if !recovering {
        if let Some(command) = last_stand(view, controller) {
            commands.push(command);
        }
        commands.extend(queue_units(view, controller));
    }
    commands.extend(set_rally_points(view, controller));
    if let Some(command) = put_harvesters_to_work(view, controller) {
        commands.push(command);
    }
    commands.extend(command_army(view, controller));
    commands
}

/// Updates the memory of other players' buildings and the waypoints from what the player sees.
fn remember(view: &View<'_>, controller: &mut AiController) {
    let seen: BTreeSet<SimId> = view
        .enemy_buildings
        .iter()
        .map(|building| building.id)
        .collect();
    for building in &view.enemy_buildings {
        controller.known.insert(
            building.id,
            KnownBuilding {
                owner: building.owner,
                footprint: building.footprint,
                kind: building.kind,
            },
        );
    }
    // A remembered building whose cells are seen but which is not seen is gone.
    let gone: Vec<SimId> = controller
        .known
        .iter()
        .filter(|(id, known)| !seen.contains(id) && view.vision.sees_footprint(known.footprint))
        .map(|(id, _)| *id)
        .collect();
    for id in gone {
        controller.known.remove(&id);
        controller.air_screens.remove(&id);
        controller.building_jobs.remove(&id);
        controller.failures.remove(&id);
    }
    controller.seen_air |= view
        .enemy_units
        .iter()
        .any(|u| view.definitions.domain(u.kind) == Domain::Air);
    let now = view.living_owned_ids.clone();
    if controller.observed_units.iter().any(|(id, cell)| {
        !now.contains(id)
            && !view.enemy_units.iter().any(|enemy| {
                enemy.armed
                    && chebyshev(enemy.mobile.cell, *cell)
                        <= strategy::range(view.definitions, enemy.kind)
                            .div_ceil(crate::SUBCELLS_PER_CELL as u32)
            })
            && !view.enemy_buildings.iter().any(|enemy| {
                view.definitions
                    .building_combat(enemy.kind)
                    .is_some_and(|terms| {
                        terms
                            .weapons(false)
                            .into_iter()
                            .flatten()
                            .filter_map(|weapon| view.definitions.weapon(weapon))
                            .any(|weapon| {
                                weapon.damage > 0
                                    && enemy.footprint.distance_to_cell(*cell)
                                        <= weapon.range.div_ceil(crate::SUBCELLS_PER_CELL as u32)
                            })
                    })
            })
    }) {
        controller.unseen_loss = true;
    }
    controller.observed_units = now
        .into_iter()
        .map(|id| {
            let cell = view
                .own_units
                .iter()
                .find(|unit| unit.id == id)
                .map(|unit| unit.mobile.cell)
                .or_else(|| controller.observed_units.get(&id).copied())
                .unwrap_or(controller.start);
            (id, cell)
        })
        .collect();
    let stamp = view.tick.saturating_add(1);
    for waypoint in &mut controller.waypoints {
        if view.vision.is_visible(waypoint.cell) {
            waypoint.seen = stamp;
        }
    }
}

// --- Construction ---------------------------------------------------------------------------

/// Recover a lane after its actual producer or prerequisite is lost. These are the same
/// predicates that pause an incomplete head in `economy::advance_production`, evaluated
/// against owned buildings. Temporary outages, credits, power, queue capacity and build
/// limits are not losses. Completed paid items keep their ordinary placement/exit path.
fn cancel_blocked_head(view: &View<'_>) -> Option<Command> {
    view.state.queues().find_map(|(_, queue)| {
        let head = queue.first()?;
        let terms = view.definitions.production(head.item)?;
        if head.progress >= terms.build_time {
            return None;
        }
        let lost_producer = !view
            .own_kinds
            .iter()
            .any(|&kind| produces(view.definitions, kind, head.item));
        let lost_prerequisite = !view.state.unlock_all()
            && crate::economy_commands::missing_prerequisite(
                view.definitions,
                &view.own_kinds,
                head.item,
            )
            .is_some();
        (lost_producer || lost_prerequisite)
            .then_some(Command::CancelProduction { item: head.item })
    })
}

fn pending_unit_credit_need(view: &View<'_>) -> i64 {
    view.state
        .queues()
        .flat_map(|(_, queue)| queue.iter())
        .filter(|head| matches!(head.item, ProductionItem::Unit(_)))
        .filter(|head| production_can_advance(view, head.item))
        .filter_map(|head| {
            view.definitions
                .production(head.item)
                .map(|terms| terms.cost.saturating_sub(head.charged))
        })
        .fold(0_i64, i64::saturating_add)
}

fn production_can_advance(view: &View<'_>, item: ProductionItem) -> bool {
    view.own_kinds
        .iter()
        .any(|&kind| produces(view.definitions, kind, item))
        && (view.state.unlock_all()
            || crate::economy_commands::missing_prerequisite(
                view.definitions,
                &view.own_kinds,
                item,
            )
            .is_none())
}

fn outstanding_cost(view: &View<'_>, item: &crate::QueueItem) -> i64 {
    view.definitions
        .production(item.item)
        .map_or(0, |terms| terms.cost.saturating_sub(item.charged))
}

fn pending_production_cost(view: &View<'_>) -> i64 {
    view.state
        .queues()
        .flat_map(|(_, q)| q.iter())
        .map(|q| outstanding_cost(view, q))
        .fold(0, i64::saturating_add)
}

/// No-income recovery replaces an unaffordable army head and removes unpaid
/// army tails with ordinary last-matching cancellations. Prefer the largest
/// source-cost finisher the finite budget can fully fund; this is an investment
/// policy, not a damage ranking. Retained work reserves its entire remaining cost.
fn recover_unit_credits(view: &View<'_>, controller: &AiController) -> Vec<Command> {
    if !view.ai.rules_difficulty
        || strategy::service_income(view)
        || !controller.broken_bridges.is_empty()
    {
        return Vec::new();
    }
    let mut alternatives: Vec<_> = view
        .ai
        .army
        .iter()
        .copied()
        .filter(|&kind| view.queueable(ProductionItem::Unit(kind)))
        .filter_map(|kind| {
            view.definitions
                .production(ProductionItem::Unit(kind))
                .map(|p| (p.cost, kind))
        })
        .collect();
    alternatives.sort_unstable();
    alternatives.dedup();
    let no_unit_work = view
        .state
        .queues()
        .flat_map(|(_, queue)| queue.iter())
        .all(|q| !matches!(q.item, ProductionItem::Unit(_)));
    for (cost, kind) in alternatives.into_iter().rev() {
        let mut cancellations = Vec::new();
        let mut refund = 0_i64;
        let mut protected = 0_i64;
        let mut heads = Vec::new();
        for (_, queue) in view.state.queues() {
            let unfunded = queue.first().is_some_and(|head| {
                view.definitions.production(head.item).is_some_and(|terms| {
                    head.progress < terms.build_time
                        && terms.cost.saturating_sub(head.charged) > view.state.credits()
                })
            });
            for (at, item) in queue.iter().enumerate().rev() {
                let army = matches!(item.item, ProductionItem::Unit(k)
                    if view.ai.army.contains(&k)
                        && view.definitions.harvester(k).is_none()
                        && view.definitions.metadata(item.item).is_some_and(|m|
                            !matches!(m.role, crate::Role::Dog | crate::Role::Engineer
                                | crate::Role::Spy | crate::Role::Mcv | crate::Role::Transport
                                | crate::Role::Repair | crate::Role::ChronoMiner)));
                let construction = no_unit_work
                    && queue.len() == 1
                    && at == 0
                    && item.charged > 0
                    && matches!(item.item, ProductionItem::Building(k) if !income_construction(view, k));
                let cancel = unfunded
                    && (army || construction)
                    && view.definitions.production(item.item).is_some_and(|terms| {
                        item.progress < terms.build_time
                            && if construction {
                                true
                            } else if at == 0 {
                                terms.cost > cost
                            } else {
                                item.charged == 0
                            }
                    });
                if cancel {
                    if at == 0 {
                        heads.push((item.charged, item));
                        protected = protected.saturating_add(outstanding_cost(view, item));
                    }
                } else {
                    protected = protected.saturating_add(outstanding_cost(view, item));
                }
                if cancel && at != 0 {
                    cancellations.push(Command::CancelProduction { item: item.item });
                }
            }
        }
        // Recover the largest real paid refund first, stopping as soon as the
        // entire retained FIFO and new item are financed. Other paid heads stay.
        heads.sort_unstable_by_key(|(paid, item)| (std::cmp::Reverse(*paid), item.item));
        let mut replaced = BTreeSet::new();
        for (paid, item) in heads {
            refund = refund.saturating_add(paid);
            protected = protected.saturating_sub(outstanding_cost(view, item));
            replaced.insert(item.item);
            if view.state.credits().saturating_add(refund) >= protected.saturating_add(cost) {
                break;
            }
        }
        // Unpaid tails must precede a matching paid head under public last-match
        // cancellation. Their canonical queue/reverse order is already above.
        cancellations.extend(
            replaced
                .iter()
                .map(|&item| Command::CancelProduction { item }),
        );
        // Leave room for the existing outer bridge action and ready placement.
        if replaced.is_empty()
            || cancellations.len().saturating_add(1) > MAX_AI_COMMANDS.saturating_sub(2)
            || view.state.credits().saturating_add(refund) < protected.saturating_add(cost)
            || !strategy::production_finisher(view, kind)
        {
            continue;
        }
        cancellations.push(Command::QueueProduction {
            item: ProductionItem::Unit(kind),
        });
        return cancellations;
    }
    Vec::new()
}

/// Read-only FIFO accounting for final bounded intents. Simulate public
/// last-matching refunds, and reserve current/same-turn production before adding
/// another unit. The actual commands still perform every validation and charge.
fn retain_funded_unit_intents(view: &View<'_>, commands: &mut Vec<Command>, cadence: u32) {
    if !view.ai.rules_difficulty || strategy::service_income(view) {
        return;
    }
    // Only the bounded candidate prefix can supply refunds or claim credits.
    commands.truncate(MAX_AI_COMMANDS);
    let rules = view.definitions.rules().map(|r| &r.economy);
    let repair_step = |building: &&BuildingSnap| {
        let rules = rules?;
        let max = view.definitions.building_combat(building.kind)?.hit_points;
        (building.hp < max).then(|| {
            let price = view
                .definitions
                .production(ProductionItem::Building(building.kind))
                .map_or(0, |terms| terms.cost);
            crate::repair_step_cost(rules, price, max, view.state.super_me)
        })
    };
    // Commands apply next tick. Existing repairs due in this Economy stage
    // cannot be stopped retroactively, so new queues cannot claim that money.
    let due_now = rules.is_some_and(|r| {
        view.tick
            .is_multiple_of(u64::from(r.repair_interval.max(1).unsigned_abs()))
    });
    let unavoidable = if due_now {
        view.own_buildings
            .iter()
            .filter(|b| view.state.repairing().contains(&b.id))
            .filter_map(repair_step)
            .fold(0_i64, i64::saturating_add)
    } else {
        0
    };
    let project = |commands: &mut Vec<Command>| {
        let mut queues: BTreeMap<_, _> = view
            .state
            .queues()
            .map(|(key, q)| (key, q.to_vec()))
            .collect();
        let mut cash = view.state.credits().saturating_sub(unavoidable).max(0);
        commands.retain(|command| {
            match command {
                Command::CancelProduction { item } => {
                    if let Some(key) = view.definitions.queue_of(*item)
                        && let Some(queue) = queues.get_mut(&key)
                        && let Some(at) = queue.iter().rposition(|q| q.item == *item)
                    {
                        cash = cash.saturating_add(queue.remove(at).charged);
                    }
                }
                Command::QueueProduction { item } => {
                    if let Some(terms) = view.definitions.production(*item) {
                        if matches!(item, ProductionItem::Unit(_)) {
                            let reserved = queues
                                .values()
                                .flat_map(|q| q.iter())
                                .map(|q| outstanding_cost(view, q))
                                .fold(0_i64, i64::saturating_add);
                            if cash < reserved.saturating_add(terms.cost) {
                                return false;
                            }
                        }
                        if let Some(key) = view.definitions.queue_of(*item) {
                            queues
                                .entry(key)
                                .or_default()
                                .push(crate::QueueItem::new(*item));
                        }
                    }
                }
                _ => {}
            }
            true
        });
        (queues, cash)
    };
    let (queues, cash) = project(commands);
    // A repair step spends the same finite wallet as these ordinary queues.
    // Do not make a fully financed paid recruit require another sale/refund.
    // Completed heads need no further credits; income keeps normal repairs.
    if !queues
        .values()
        .flatten()
        .any(|q| matches!(q.item, ProductionItem::Unit(_)) && outstanding_cost(view, q) > 0)
    {
        return;
    }
    let Some(rules) = rules else {
        return;
    };
    let mut repairing = view.state.repairing().clone();
    for command in commands.iter() {
        if let Command::RepairBuilding { building } = command
            && !repairing.remove(building)
        {
            repairing.insert(*building);
        }
    }
    let repair_cost = view
        .own_buildings
        .iter()
        .filter(|b| repairing.contains(&b.id))
        .filter_map(repair_step)
        .fold(0_i64, i64::saturating_add);
    let interval = u64::from(rules.repair_interval.max(1).unsigned_abs());
    // A repair enabled at tick+1 can charge through the next decision tick;
    // that turn's ordinary off command also applies one tick later.
    let steps = view.tick.saturating_add(u64::from(cadence)) / interval - view.tick / interval;
    let repair_cost = repair_cost.saturating_mul(i64::try_from(steps).unwrap_or(i64::MAX));
    let reserved = queues
        .values()
        .flatten()
        .map(|q| outstanding_cost(view, q))
        .fold(0_i64, i64::saturating_add);
    if cash >= reserved.saturating_add(repair_cost) {
        return;
    }
    commands.retain(|c| {
        !matches!(c, Command::RepairBuilding { building }
            if !view.state.repairing().contains(building))
    });
    let mut pauses: Vec<_> = view
        .own_buildings
        .iter()
        .filter(|b| repairing.contains(&b.id) && view.state.repairing().contains(&b.id))
        .take(MAX_AI_COMMANDS)
        .map(|b| Command::RepairBuilding { building: b.id })
        .collect();
    // Pausing the competing spend needs actual command capacity. Keep the
    // relative FIFO of other intents, then recompute the real emitted prefix:
    // an insertion must not leave a dropped tail refund in the projection.
    pauses.append(commands);
    pauses.truncate(MAX_AI_COMMANDS);
    let _ = project(&mut pauses);
    *commands = pauses;
}

/// Reserve the whole existing unit FIFO, or the next legal replacement's cost.
/// With no income, new construction must not claim the same finite credits.
fn combat_credit_reserve(view: &View<'_>) -> i64 {
    let next = view
        .ai
        .army
        .iter()
        .copied()
        .filter(|&kind| view.buildable(ProductionItem::Unit(kind)))
        .filter_map(|kind| {
            view.definitions
                .production(ProductionItem::Unit(kind))
                .map(|terms| terms.cost)
        })
        .min()
        .unwrap_or(0);
    pending_unit_credit_need(view).max(next)
}

fn income_construction(view: &View<'_>, kind: BuildingKind) -> bool {
    view.definitions
        .building(kind)
        .is_some_and(|b| b.power_produced > 0 || b.credits_per_ore.is_some())
        || view
            .definitions
            .metadata(ProductionItem::Building(kind))
            .is_some_and(|m| m.cash_amount > 0)
}

/// Ordinary exact-refund cancellation, only when finite credits really starve
/// otherwise legal combat production. Keep completed heads and income recovery.
fn cancel_starved_construction(view: &View<'_>) -> Option<Command> {
    if !view.ai.rules_difficulty
        || strategy::service_income(view)
        || view.state.credits() >= pending_unit_credit_need(view)
    {
        return None;
    }
    view.state.queues().find_map(|(_, queue)| {
        let head = queue.first()?;
        let ProductionItem::Building(kind) = head.item else {
            return None;
        };
        let terms = view.definitions.production(head.item)?;
        (head.charged > 0 && head.progress < terms.build_time && !income_construction(view, kind))
            .then_some(Command::CancelProduction { item: head.item })
    })
}

/// What a turn does about the player's base unit.
enum BaseUnit {
    /// It owns none, or the one it owns cannot deploy where it stands: the turn goes on as usual.
    None,
    /// Deploy it, and do nothing else in this turn.
    Deploy(Command),
    /// It cannot see the yard's footprint yet (the first turn of a match comes before the first
    /// `Visibility` stage): do nothing in this turn, so that its units stay where they are.
    Wait,
}

/// `Deploy` for the base unit of a side that the player owns (RA1: the MCV a country starts
/// with), if deploying where it stands would pass the command's validation now and still in the
/// next tick: the unit stands still, the yard's footprint (its origin is the unit's cell) lies
/// on the map on buildable ground that the player sees and keeps seeing, nothing but the unit
/// itself is in it, and beside it, as far as the player sees, stand at most its own units
/// without an order (its starting units, which this turn does not move). A base unit that
/// cannot deploy where it stands is left alone, and the rest of the turn goes on as usual;
/// except that in its first two turns a player that does not see the footprint yet waits.
fn deploy_base_unit(view: &View<'_>, controller: &AiController) -> BaseUnit {
    let Some(rules) = view.definitions.rules() else {
        return BaseUnit::None;
    };
    let early = view.tick < 2 * u64::from(controller.difficulty.think_interval.max(1));
    for unit in &view.own_units {
        let Some(def) = rules
            .side_of_base_unit(unit.kind)
            .and_then(|side| view.definitions.building(side.construction_yard))
        else {
            continue;
        };
        if unit.has_order || unit.mobile.step.is_some() {
            continue;
        }
        let footprint = Footprint {
            origin: unit.mobile.cell,
            width: def.width,
            height: def.height,
        };
        if !footprint.inside(view.grid) {
            continue;
        }
        if !footprint.cells().all(|cell| view.vision.is_visible(cell)) {
            if early {
                return BaseUnit::Wait;
            }
            continue;
        }
        let clear = |cell: CellPos| {
            view.grid.building(cell).is_none()
                && view.grid.occupant(cell).is_none_or(|by| by == unit.id)
        };
        // One of the player's own units that stands still without any order: it does not move
        // before the command is applied, since this turn gives no other command.
        let stands = |by: SimId| {
            view.own_units.iter().any(|other| {
                other.id == by
                    && other.idle()
                    && other.mobile.step.is_none()
                    && other.harvest.is_none()
            })
        };
        let fits = footprint.cells().all(|cell| {
            view.stays_visible(cell)
                && view
                    .grid
                    .cell(cell)
                    // Nothing is built on ore (RA5): a base unit a crate gave it may stand
                    // on ore that spread.
                    .is_some_and(|terrain| terrain.buildable && terrain.ore == 0)
                && clear(cell)
        }) && footprint
            .ring()
            .filter(|&cell| view.grid.contains(cell) && view.vision.is_visible(cell))
            .all(|cell| view.grid.holders(cell).into_iter().flatten().all(stands));
        if fits {
            return BaseUnit::Deploy(Command::Deploy { unit: unit.id });
        }
    }
    BaseUnit::None
}

/// `PlaceBuilding` for a building that is ready at the head of one of the player's queues, if a
/// place is found: the first ready building, in the order of the queues, that has one (with the
/// two lanes of RA2 a building and a defence may be ready at once; one is placed per turn).
fn place_ready_building(view: &View<'_>, controller: &AiController) -> Option<Command> {
    view.state
        .queues()
        .filter_map(|(_, items)| {
            let head = items.first()?;
            let ProductionItem::Building(kind) = head.item else {
                return None;
            };
            let terms = view.definitions.production(head.item)?;
            (head.progress >= terms.build_time).then_some(kind)
        })
        .find_map(|ready| {
            let origin = find_placement(view, controller, ready)?;
            Some(Command::PlaceBuilding {
                building: ready,
                origin,
            })
        })
}

/// A footprint origin for a building of type `kind` near the player's base that passes the
/// placement geometry now ([`check_placement_site`]); the actual command also requires a
/// finished queue item. The site stays valid until the command is
/// validated in the next tick, unless one of the player's buildings is destroyed in between
/// (docs/DECISIONS.md D43):
///
/// * the footprint is visible now and stays visible in the next tick ([`View::stays_visible`]);
/// * no unit holds a cell of the footprint or a visible cell around it, and none of the player's
///   own units holds a cell around it that the player does not see, so none of them can enter the
///   footprint in one tick (an enemy unit unseen next to it can, and the command is then
///   rejected as `Occupied`, which reveals only a unit the player then sees);
/// * no building covers a visible cell around it, and it keeps one free cell to the player's
///   buildings and to the gathering point, and two to the player's buildings that a unit may leave
///   in the tick in between (the head of their queue completes then, or they hold free units), so
///   such a unit cannot step into it;
/// * some visible cell around it is passable, so units can reach it.
///
/// Origins are tried nearest to the start position first; a refinery nearest to visible ore first.
fn find_placement(
    view: &View<'_>,
    controller: &AiController,
    kind: BuildingKind,
) -> Option<CellPos> {
    let preferred = controller.naval_site.filter(|_| {
        view.definitions
            .metadata(ProductionItem::Building(kind))
            .is_some_and(|m| m.role == Role::Shipyard)
    });
    find_placement_where(view, controller, kind, |origin| {
        preferred.is_none_or(|site| site == origin)
    })
}

fn find_placement_where(
    view: &View<'_>,
    controller: &AiController,
    kind: BuildingKind,
    mut accepts: impl FnMut(CellPos) -> bool,
) -> Option<CellPos> {
    let def = view.definitions.building(kind)?;
    let (width, height) = (i32::from(def.width.max(1)), i32::from(def.height.max(1)));
    // The building's own reach, from the buildings beside which one may be placed (RA2:
    // `adjacent` and `base_normal`; without a rules file the economy file's distance from any).
    let reach = i32::try_from(placement_reach(view.definitions, kind)).unwrap_or(0);
    let mut origins: BTreeSet<CellPos> = BTreeSet::new();
    for building in view
        .own_buildings
        .iter()
        .filter(|building| extends_base(view.definitions, building.kind))
    {
        let footprint = building.footprint;
        let (x0, y0) = (footprint.origin.x, footprint.origin.y);
        let (x1, y1) = (
            x0.saturating_add(i32::from(footprint.width.max(1)) - 1),
            y0.saturating_add(i32::from(footprint.height.max(1)) - 1),
        );
        for y in y0.saturating_sub(reach + height - 1)..=y1.saturating_add(reach) {
            for x in x0.saturating_sub(reach + width - 1)..=x1.saturating_add(reach) {
                origins.insert(CellPos::new(x, y));
            }
        }
    }
    let footprint_at = |origin: CellPos| Footprint {
        origin,
        width: def.width,
        height: def.height,
    };
    // The gathering point stays free, so that produced units can reach it.
    let spaced = |footprint: &Footprint| {
        footprint.distance_to_cell(controller.gather) >= BUILDING_GAP
            && view.own_buildings.iter().all(|building| {
                let gap = footprint.distance(&building.footprint);
                let leaves = building.producer.is_some() || !building.pending.is_empty();
                gap >= if leaves { PRODUCER_GAP } else { BUILDING_GAP }
            })
    };
    let ore: Vec<CellPos> = if def.credits_per_ore.is_some() {
        visible_ore(view)
    } else {
        Vec::new()
    };
    let avoid_naval_fire = view.ai.rules_difficulty
        && view
            .definitions
            .metadata(ProductionItem::Building(kind))
            .is_some_and(|meta| meta.role == Role::Shipyard);
    let mut candidates: Vec<(bool, u32, u32, CellPos)> = origins
        .into_iter()
        .map(footprint_at)
        .filter(|footprint| footprint.inside(view.grid) && spaced(footprint))
        .map(|footprint| {
            let to_ore = ore
                .iter()
                .map(|&cell| footprint.distance_to_cell(cell))
                .min()
                .unwrap_or(0);
            (
                avoid_naval_fire && strategy::naval_site_exposed(view, kind, footprint),
                to_ore,
                octile_distance(footprint.origin, controller.start),
                footprint.origin,
            )
        })
        .collect();
    candidates.sort_unstable_by_key(|&(exposed, to_ore, distance, origin)| {
        (exposed, to_ore, distance, origin.y, origin.x)
    });
    let owned: Vec<(BuildingKind, Footprint)> = view
        .own_buildings
        .iter()
        .map(|building| (building.kind, building.footprint))
        .collect();
    let own_cells: BTreeSet<CellPos> = view
        .own_units
        .iter()
        .flat_map(|unit| [unit.mobile.cell, unit.mobile.next_centre()])
        .collect();
    candidates
        .into_iter()
        .map(|(.., origin)| origin)
        .find(|&origin| {
            let footprint = footprint_at(origin);
            let ring: Vec<CellPos> = footprint
                .ring()
                .filter(|&cell| view.grid.contains(cell))
                .collect();
            // Visible first, so that nothing below reads a cell the player does not see.
            footprint.cells().all(|cell| view.vision.is_visible(cell))
                && footprint.cells().all(|cell| view.stays_visible(cell))
                && footprint
                    .cells()
                    .all(|cell| view.grid.occupant(cell).is_none())
                && ring.iter().all(|&cell| {
                    if view.vision.is_visible(cell) {
                        view.grid.occupant(cell).is_none() && view.grid.building(cell).is_none()
                    } else {
                        !own_cells.contains(&cell)
                    }
                })
                && ring
                    .iter()
                    .any(|&cell| view.vision.is_visible(cell) && view.grid.is_passable(cell))
                && check_placement_site(
                    view.grid,
                    view.definitions,
                    view.state,
                    owned.iter().copied(),
                    kind,
                    origin,
                )
                .is_ok()
                && accepts(origin)
        })
}

/// The cells with ore the player sees now.
fn visible_ore(view: &View<'_>) -> Vec<CellPos> {
    let (width, height) = (grid_side(view.grid.width()), grid_side(view.grid.height()));
    (0..height)
        .flat_map(|y| (0..width).map(move |x| CellPos::new(x, y)))
        .filter(|&cell| {
            view.vision.is_visible(cell) && view.grid.cell(cell).is_some_and(|cell| cell.ore > 0)
        })
        .collect()
}

/// A new worker needs an observed field in the same public-terrain component
/// as an actual producer and refinery. Existing cargo, workers and queued heads
/// are left alone. Temporary traffic and unseen building occupancy are not facts
/// available to this planning query; ordinary movement still enforces them.
fn worker_job(view: &View<'_>) -> bool {
    let ore: BTreeSet<_> = visible_ore(view).into_iter().collect();
    if ore.is_empty() {
        return false;
    }
    let domain = view.definitions.domain(view.ai.harvester);
    let own_cells: BTreeSet<_> = view
        .own_buildings
        .iter()
        .flat_map(|b| b.footprint.cells())
        .collect();
    let passable = |cell: CellPos| {
        view.grid.cell(cell).is_some_and(|c| match domain {
            Domain::Land => c.passable,
            Domain::Amphibious => c.passable || c.water,
            Domain::Naval => c.water,
            Domain::Air => true,
        }) && !own_cells.contains(&cell)
            && !(view.vision.is_visible(cell) && view.grid.building(cell).is_some())
    };
    let docks: BTreeSet<_> = view
        .own_buildings
        .iter()
        .filter(|b| {
            view.definitions
                .building(b.kind)
                .is_some_and(|def| def.credits_per_ore.is_some())
        })
        .flat_map(|b| b.footprint.ring())
        .filter(|&cell| passable(cell))
        .collect();
    let mut visited = BTreeSet::new();
    for start in view
        .own_buildings
        .iter()
        .filter(|b| {
            produces(
                view.definitions,
                b.kind,
                ProductionItem::Unit(view.ai.harvester),
            )
        })
        .flat_map(|b| b.footprint.ring())
        .filter(|&cell| passable(cell))
    {
        if !visited.insert(start) {
            continue;
        }
        let mut queue = std::collections::VecDeque::from([start]);
        let (mut field, mut dock) = (false, false);
        while let Some(cell) = queue.pop_front() {
            field |= ore.contains(&cell);
            dock |= docks.contains(&cell);
            if field && dock {
                return true;
            }
            // Cardinal connectivity is equivalent to the movement graph's
            // diagonals, which require a passable orthogonal neighbour.
            for (dx, dy) in [(0, -1), (-1, 0), (1, 0), (0, 1)] {
                let next = CellPos::new(cell.x.saturating_add(dx), cell.y.saturating_add(dy));
                if passable(next) && visited.insert(next) {
                    queue.push_back(next);
                }
            }
        }
    }
    false
}

/// Whether the player can neither fight nor pay for a fighter, though it could produce one: it has
/// no armed unit, none of its harvesters is working (all have stopped, or it has none), it owns the
/// producer and prerequisites of some army unit type, and its credits are below what the cheapest
/// army unit in its queues still costs (or, with none queued, the cost of the cheapest one it can
/// produce). Without income its credits will not grow, so a match between such players would never
/// end (docs/DECISIONS.md D43).
fn broke(view: &View<'_>) -> bool {
    if view
        .own_units
        .iter()
        .chain(&view.recoverable_service)
        .any(|unit| view.fights(unit))
        || view.own_units.iter().any(|unit| {
            unit.harvest
                .is_some_and(|state| state != HarvestState::Idle)
        })
    {
        return false;
    }
    let producible: Vec<i64> = view
        .ai
        .army
        .iter()
        .filter(|&&kind| view.buildable(ProductionItem::Unit(kind)))
        .filter_map(|&kind| view.definitions.production(ProductionItem::Unit(kind)))
        .map(|terms| terms.cost)
        .collect();
    let Some(&cheapest) = producible.iter().min() else {
        // Selling cannot help: no army unit can be produced.
        return false;
    };
    let queued = view
        .state
        .queues()
        .flat_map(|(_, items)| items.iter())
        .filter(|queued| matches!(queued.item, ProductionItem::Unit(kind) if view.ai.army.contains(&kind)))
        .filter_map(|queued| {
            view.definitions
                .production(queued.item)
                .map(|terms| terms.cost.saturating_sub(queued.charged))
        })
        .min();
    view.state.credits() < queued.unwrap_or(cheapest)
}

/// While broke, sells one building to fund army units: never one needed to produce an army unit
/// type (a producer or prerequisite of one), and a building that produces buildings only when
/// nothing else is left to sell; otherwise the one with the highest sell value (then the lowest
/// id). It does not rebuild while broke, so selling does not feed rebuilding.
fn sell_for_army(view: &View<'_>) -> Option<Command> {
    // The producers and prerequisites of the army: a building that produces one of its unit
    // types, a prerequisite of one, or a member of one of its prerequisite groups.
    let mut needed: BTreeSet<BuildingKind> = BTreeSet::new();
    for &kind in &view.ai.army {
        let item = ProductionItem::Unit(kind);
        if let Some(terms) = view.definitions.production(item) {
            needed.insert(terms.producer);
            needed.extend(terms.prerequisites.iter().copied());
        }
        needed.extend(
            view.own_kinds
                .iter()
                .copied()
                .filter(|&owned| produces(view.definitions, owned, item)),
        );
        if let (Some(entry), Some(rules)) =
            (view.definitions.metadata(item), view.definitions.rules())
        {
            for group in entry
                .prerequisite_groups
                .iter()
                .filter_map(|&group| rules.group(group))
            {
                needed.extend(group.buildings.iter().copied());
            }
        }
    }
    let builds = |kind: BuildingKind| {
        view.definitions
            .buildings()
            .iter()
            .any(|def| view.definitions.same_factory(kind, def.production.producer))
    };
    // What cannot be sold stays (RA2: walls).
    let sellable = |kind: BuildingKind| {
        view.definitions.rules().is_none()
            || view
                .definitions
                .metadata(ProductionItem::Building(kind))
                .is_none_or(|entry| entry.sellable)
    };
    view.own_buildings
        .iter()
        .filter(|building| !needed.contains(&building.kind) && sellable(building.kind))
        .max_by_key(|building| {
            let value = view.definitions.refund(building.kind);
            (
                !builds(building.kind),
                value,
                std::cmp::Reverse(building.id),
            )
        })
        .map(|building| Command::SellBuilding {
            building: building.id,
        })
}

/// The last stand (RA4, docs/DECISIONS.md R53). A player that has no unit that fights and no
/// harvester at work, and whose credits, units, buildings and queues have not changed for
/// [`LAST_STAND_PATIENCE`] ticks, will never change anything again: it earns nothing, and it
/// can pay for nothing or (with credits that stand still) can use nothing it pays for, such as
/// a finished building that has no site; neither can its opponent be made to end the match
/// (two such players would stand for ever). It then sells its buildings, the one with the highest refund first
/// (then the lowest id), one a turn and only in the turn of its player id
/// ([`LAST_STAND_TURN`]): the survivors of a crewed building are armed units that attack with
/// the next wave, and a player without a building is defeated in a short game. It stops when
/// it has a unit that fights or a harvester at work again.
fn last_stand(view: &View<'_>, controller: &mut AiController) -> Option<Command> {
    let helpless = !view.own_units.iter().any(|unit| {
        unit.harvest
            .is_some_and(|state| state != HarvestState::Idle)
    }) && !view
        .own_units
        .iter()
        .chain(&view.recoverable_service)
        .any(|unit| strategy::can_finish(view, controller, unit));
    if !helpless {
        controller.last_stand = false;
        controller.stalled = (0, view.tick);
        return None;
    }
    if !controller.last_stand {
        let mut hasher = Fnv1a64::new();
        hasher.write(&view.state.credits().to_le_bytes());
        hasher.write_u64(view.own_units.len() as u64);
        hasher.write_u64(view.own_buildings.len() as u64);
        // The progress of its queues and what was charged for it; not the fraction of a step
        // of progress, which a queue that cannot be paid for carries from tick to tick.
        for (_, items) in view.state.queues() {
            for item in items {
                hasher.write_u32(item.progress);
                hasher.write(&item.charged.to_le_bytes());
            }
        }
        let had = hasher.finish();
        if had != controller.stalled.0 {
            controller.stalled = (had, view.tick);
            return None;
        }
        if view.tick.saturating_sub(controller.stalled.1) < LAST_STAND_PATIENCE {
            return None;
        }
        if let Some(&unit) = view.recoverable_garrisons.first() {
            return Some(Command::Deploy { unit });
        }
        controller.last_stand = true;
    }
    if (view.tick / LAST_STAND_TURN) % 16 != u64::from(view.player.0 % 16) {
        return None;
    }
    let sellable = |kind: BuildingKind| {
        view.definitions.rules().is_none()
            || view
                .definitions
                .metadata(ProductionItem::Building(kind))
                .is_none_or(|entry| entry.sellable)
    };
    view.own_buildings
        .iter()
        .filter(|building| sellable(building.kind))
        .max_by_key(|building| {
            (
                view.definitions.refund(building.kind),
                std::cmp::Reverse(building.id),
            )
        })
        .map(|building| Command::SellBuilding {
            building: building.id,
        })
}

/// For each lane of buildings (one without a rules file; with one the buildings and the
/// defences of the Construction Yard, RA2): the next building of the build order in that lane
/// that the player does not own as often as the build order lists it up to that entry, queued if
/// its lane is empty. A power building is queued instead if that building would leave the
/// player without enough power, counting what the buildings in its queues will produce and
/// consume, or if the build order is complete and the player has low power; a lane whose
/// building must wait for power queues nothing.
fn queue_buildings(view: &View<'_>, controller: &AiController) -> Vec<Command> {
    let order = &view.ai.build_order;
    let lane = |kind: BuildingKind| view.definitions.lane(ProductionItem::Building(kind));
    let mut needed: BTreeMap<Lane, BuildingKind> = BTreeMap::new();
    for (index, &kind) in order.iter().enumerate() {
        if let Some(meta) = view.definitions.metadata(ProductionItem::Building(kind))
            && ((!view.superweapons && meta.role.major_superweapon())
                || (meta.role == Role::Shipyard && controller.naval_goal.is_none()))
        {
            continue;
        }
        let listed = order[..=index]
            .iter()
            .filter(|&&other| other == kind)
            .count();
        if view.owned(kind) < listed {
            needed.entry(lane(kind)).or_insert(kind);
        }
    }
    if let Some(rule) = strategy::difficulty(view, controller) {
        let side = view
            .state
            .country()
            .and_then(|c| view.definitions.rules()?.country(c))
            .map(|c| c.side);
        let wanted = match side {
            Some(crate::Side::Allied) => rule.allied_base_defences,
            Some(crate::Side::Soviet) => rule.soviet_base_defences,
            None => 0,
        };
        let count = view
            .own_buildings
            .iter()
            .filter(|b| view.ai.defences.contains(&b.kind))
            .count();
        let eligible: Vec<_> = view
            .ai
            .defences
            .iter()
            .copied()
            .filter(|&k| view.buildable(ProductionItem::Building(k)))
            .filter(|&k| controller.seen_air || !strategy::only_aa_building(view.definitions, k))
            .collect();
        // Reserve one slot for each later-tier defence, including a country's special.
        let cap = (wanted.unsigned_abs() as usize)
            .saturating_sub(view.ai.defences.len().saturating_sub(eligible.len()));
        if count < cap {
            let candidate = eligible
                .into_iter()
                .min_by_key(|&k| (view.owned(k), std::cmp::Reverse(k)));
            if let Some(k) = candidate {
                let slot = needed.entry(lane(k)).or_insert(k);
                // A later-tech item must not reserve an otherwise idle
                // defence lane before its prerequisites exist. Ready build-order
                // items keep their priority over the sourced defence count.
                if !view.buildable(ProductionItem::Building(*slot)) {
                    *slot = k;
                }
            }
        }
    }
    let power_of = |kind: BuildingKind| {
        view.definitions.building(kind).map_or(0, |def| {
            def.power_produced.saturating_sub(def.power_consumed)
        })
    };
    // The power to spare once every building in the queues stands.
    let mut spare = view
        .state
        .queues()
        .flat_map(|(_, items)| items.iter())
        .filter_map(|queued| match queued.item {
            ProductionItem::Building(kind) => Some(power_of(kind)),
            ProductionItem::Unit(_) => None,
        })
        .fold(
            view.state
                .power_produced()
                .saturating_sub(view.state.power_consumed()),
            i32::saturating_add,
        );
    let mut commands = Vec::new();
    let mut taken: BTreeSet<Lane> = BTreeSet::new();
    let construction_debt = view
        .state
        .queues()
        .flat_map(|(_, queue)| queue.iter())
        .filter(|item| matches!(item.item, ProductionItem::Building(_)))
        .map(|item| outstanding_cost(view, item))
        .fold(0_i64, i64::saturating_add);
    let reserve = combat_credit_reserve(view).saturating_add(construction_debt);
    let mut cash = view.state.credits();
    let mut finite_budget = (view.ai.rules_difficulty && !strategy::service_income(view))
        .then(|| view.state.credits().saturating_sub(reserve).max(0));
    let mut queue = |kind: BuildingKind, spare: &mut i32, taken: &mut BTreeSet<Lane>| {
        let item = ProductionItem::Building(kind);
        if !taken.contains(&lane(kind)) && view.queue_len(item) == 0 && view.queueable(item) {
            if let Some(budget) = &mut finite_budget {
                let Some(terms) = view.definitions.production(item) else {
                    return;
                };
                // A real current power shortage is recovery, rather than optional
                // expansion. Preserve its affordable ordinary production even
                // when later unit FIFO entries reserve more than current cash.
                let restores_power = view.state.low_power()
                    && view
                        .definitions
                        .building(kind)
                        .is_some_and(|b| b.power_produced > 0);
                if terms.cost > *budget && !(restores_power && terms.cost <= cash) {
                    return;
                }
                cash = cash.saturating_sub(terms.cost);
                *budget = cash.saturating_sub(reserve).max(0);
            }
            taken.insert(lane(kind));
            *spare = spare.saturating_add(power_of(kind));
            commands.push(Command::QueueProduction { item });
        }
    };
    let mut power = needed.is_empty() && view.state.low_power();
    for &kind in needed.values() {
        if kind != view.ai.power && spare.saturating_add(power_of(kind)) < 0 {
            power = true;
        } else {
            queue(kind, &mut spare, &mut taken);
        }
    }
    if power {
        queue(view.ai.power, &mut spare, &mut taken);
    }
    commands
}

/// Harvesters up to the difficulty's number per refinery, then armed units of the army
/// composition up to its army size, at most [`AI_UNIT_QUEUE`] items per producing building type.
fn queue_units(view: &View<'_>, controller: &mut AiController) -> Vec<Command> {
    let mut commands = Vec::new();
    let mut added = AddedUnits::default();
    let harvester = ProductionItem::Unit(view.ai.harvester);
    let refineries = view
        .own_buildings
        .iter()
        .filter(|building| {
            view.definitions
                .building(building.kind)
                .is_some_and(|def| def.credits_per_ore.is_some())
        })
        .count();
    let harvesters = view
        .own_units
        .iter()
        .filter(|unit| unit.harvest.is_some())
        .count()
        + view
            .own_buildings
            .iter()
            .flat_map(|building| building.pending.iter())
            .filter(|&&kind| view.definitions.harvester(kind).is_some())
            .count()
        + view.queued(harvester);
    let wanted = refineries.saturating_mul(controller.difficulty.harvesters_per_refinery as usize);
    if harvesters < wanted
        && (!view.ai.rules_difficulty || worker_job(view))
        && has_room(view, harvester, &mut added)
    {
        commands.push(Command::QueueProduction { item: harvester });
    }

    let army = &view.ai.army;
    let mut total = view
        .own_units
        .iter()
        .filter(|unit| view.fights(unit))
        .count()
        + army
            .iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|&kind| view.queued(ProductionItem::Unit(kind)))
            .sum::<usize>();
    let mut max = controller.difficulty.max_army as usize;
    if view.ai.rules_difficulty {
        // Keep a small tier reserve: completing tech or an airfield must still change
        // production when the surviving opening army already fills its ground quota.
        for (air, wanted) in [
            (true, view.ai.tier_reserves[0] as usize),
            (false, view.ai.tier_reserves[1] as usize),
        ] {
            let eligible = |kind| {
                if air {
                    view.definitions.domain(kind) == Domain::Air
                } else {
                    view.definitions.domain(kind) == Domain::Land
                        && view
                            .definitions
                            .metadata(ProductionItem::Unit(kind))
                            .is_some_and(|m| m.tech_level >= view.ai.advanced_tech_level as i32)
                }
            };
            let mut count = view.own_units.iter().filter(|u| eligible(u.kind)).count()
                + army
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>()
                    .iter()
                    .filter(|&&kind| eligible(kind))
                    .map(|&kind| view.queued(ProductionItem::Unit(kind)))
                    .sum::<usize>();
            if !army.iter().any(|&kind| eligible(kind)) {
                continue;
            }
            for &kind in army.iter().filter(|&&kind| eligible(kind)) {
                if count >= wanted || total >= max {
                    break;
                }
                let item = ProductionItem::Unit(kind);
                if has_room(view, item, &mut added) {
                    commands.push(Command::QueueProduction { item });
                    total += 1;
                    count += 1;
                }
            }
            max = max.saturating_sub(wanted.saturating_sub(count));
        }
    }
    // Retain the declared naval capacity before scouting/tech completes. Recruiting
    // into those slots still requires observed targets and a legal reachable coastal site.
    if view.ai.rules_difficulty
        && army
            .iter()
            .any(|&kind| view.definitions.domain(kind) == Domain::Naval)
    {
        let naval = |kind| view.definitions.domain(kind) == Domain::Naval;
        let mut count = view.own_units.iter().filter(|u| naval(u.kind)).count()
            + army
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .iter()
                .filter(|&&kind| naval(kind))
                .map(|&kind| view.queued(ProductionItem::Unit(kind)))
                .sum::<usize>();
        // A qualified bombardment route also needs its declared detector escort, even
        // when that escort's own weapon cannot damage the observed shore target.
        let detector = |kind| {
            naval(kind)
                && view
                    .definitions
                    .metadata(ProductionItem::Unit(kind))
                    .is_some_and(|meta| meta.sensors_sight > 0)
        };
        let detectors = view.own_units.iter().filter(|u| detector(u.kind)).count()
            + army
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .iter()
                .filter(|&&kind| detector(kind))
                .map(|&kind| view.queued(ProductionItem::Unit(kind)))
                .sum::<usize>();
        if controller.naval_goal.is_some()
            && detectors < view.ai.policy.naval_detector_quota as usize
            && count < view.ai.tier_reserves[2] as usize
            && total < max
            && let Some(&kind) = army.iter().find(|&&kind| detector(kind))
            && has_room(view, ProductionItem::Unit(kind), &mut added)
        {
            commands.push(Command::QueueProduction {
                item: ProductionItem::Unit(kind),
            });
            count += 1;
            total += 1;
        }
        for &kind in army.iter().filter(|&&kind| naval(kind)) {
            if controller.naval_goal.is_none() {
                break;
            }
            if !strategy::recruit(view, controller, kind) {
                continue;
            }
            if count >= view.ai.tier_reserves[2] as usize || total >= max {
                break;
            }
            let item = ProductionItem::Unit(kind);
            if has_room(view, item, &mut added) {
                commands.push(Command::QueueProduction { item });
                count += 1;
                total += 1;
            }
        }
        max = max.saturating_sub((view.ai.tier_reserves[2] as usize).saturating_sub(count));
    }
    let anti_air = |kind| {
        view.definitions
            .metadata(ProductionItem::Unit(kind))
            .is_some_and(|m| matches!(m.role, Role::Ifv | Role::Flak))
    };
    if view.ai.rules_difficulty && army.iter().any(|&k| anti_air(k)) {
        if !controller.seen_air {
            max = max.saturating_sub(army.iter().filter(|&&kind| anti_air(kind)).count());
        } else if !view.own_units.iter().any(|u| anti_air(u.kind))
            && let Some(&kind) = army
                .iter()
                .find(|&&k| anti_air(k) && view.queued(ProductionItem::Unit(k)) == 0)
            && total < max
            && has_room(view, ProductionItem::Unit(kind), &mut added)
        {
            commands.push(Command::QueueProduction {
                item: ProductionItem::Unit(kind),
            });
            total += 1;
        }
    }
    if view.ai.rules_difficulty {
        // Preserve the data's first-appearance order between producers. A busy
        // factory keeps its own place while a barracks continues admitting.
        let mut seen = BTreeSet::new();
        let producers: Vec<_> = army
            .iter()
            .filter_map(|&kind| view.queue_key(ProductionItem::Unit(kind)))
            .filter(|key| seen.insert(*key))
            .collect();
        for producer in producers {
            let start = controller.army_cursors.get(&producer).copied().unwrap_or(0);
            for step in 0..army.len() {
                if total >= max {
                    break;
                }
                let index = (start + step) % army.len();
                let item = ProductionItem::Unit(army[index]);
                if view.queue_key(item) != Some(producer)
                    || !strategy::recruit(view, controller, army[index])
                {
                    continue;
                }
                if has_room(view, item, &mut added) {
                    commands.push(Command::QueueProduction { item });
                    total += 1;
                    controller
                        .army_cursors
                        .insert(producer, (index + 1) % army.len());
                }
            }
        }
        return commands;
    }
    for step in 0..army.len() {
        if total >= max {
            break;
        }
        // Neutral fixtures retain their documented pre-RA8 traversal.
        let index = (controller.next_army + step) % army.len();
        if !strategy::recruit(view, controller, army[index]) {
            continue;
        }
        let item = ProductionItem::Unit(army[index]);
        if has_room(view, item, &mut added) {
            commands.push(Command::QueueProduction { item });
            total += 1;
            controller.next_army = (index + 1) % army.len();
        }
    }
    commands
}

/// Whether one more `item` may be queued this turn: its producing building type holds fewer than
/// [`AI_UNIT_QUEUE`] items, counting those `added` this turn, and the command would be accepted.
/// Records the item in `added` if so.
#[derive(Default)]
struct AddedUnits {
    queues: BTreeMap<QueueKey, usize>,
    docks: usize,
    credits: i64,
}

fn has_room(view: &View<'_>, item: ProductionItem, added: &mut AddedUnits) -> bool {
    let Some(producer) = view.queue_key(item) else {
        return false;
    };
    let extra = added.queues.get(&producer).copied().unwrap_or(0);
    if view.queue_len(item).saturating_add(extra) >= AI_UNIT_QUEUE || !view.queueable(item) {
        return false;
    }
    let cost = view
        .definitions
        .production(item)
        .map_or(0, |terms| terms.cost);
    if view.ai.rules_difficulty
        && !strategy::service_income(view)
        && view.state.credits()
            < pending_production_cost(view)
                .saturating_add(added.credits)
                .saturating_add(cost)
    {
        return false;
    }
    if crate::flight::needs_dock(view.definitions, item) {
        let taken = crate::flight::dock_commitments(view.definitions, view.state)
            .saturating_add(added.docks);
        let capacity = crate::flight::docks_of(view.definitions, view.own_kinds.iter().copied());
        if taken >= capacity as usize {
            return false;
        }
        added.docks += 1;
    }
    *added.queues.entry(producer).or_insert(0) += 1;
    added.credits = added.credits.saturating_add(cost);
    true
}

/// `SetRallyPoint` to the gathering point for every producer of units without a rally point.
fn set_rally_points(view: &View<'_>, controller: &AiController) -> Vec<Command> {
    view.own_buildings
        .iter()
        .filter_map(|building| {
            let old = building.producer?;
            let naval = view.ai.rules_difficulty
                && view
                    .definitions
                    .metadata(ProductionItem::Building(building.kind))
                    .is_some_and(|m| m.role == Role::Shipyard);
            let cell = if naval {
                controller.naval_goal?
            } else {
                if old.is_some() {
                    return None;
                }
                controller.gather
            };
            (old != Some(cell)).then_some(Command::SetRallyPoint {
                building: building.id,
                cell,
            })
        })
        .collect()
}

/// `Harvest` for the idle harvesters, at the refinery nearest to the start position, at most once
/// every [`HARVEST_RETRY_TICKS`]. Not for a harvester of the attack wave: an armed harvester
/// that fights as a last resort ([`View::fights`]) has no order whenever it stands and fires,
/// and a `Harvest` command then ended its attack and, with no ore to go to, left it idle far
/// from home, from where it was called back to the gathering point; a player whose only armed
/// unit it was then took a wave's patience for every few shots at the enemy's last buildings
/// (RA3, docs/DECISIONS.md R44).
fn put_harvesters_to_work(view: &View<'_>, controller: &mut AiController) -> Option<Command> {
    if view.tick < controller.next_harvest {
        return None;
    }
    let refinery = view
        .own_buildings
        .iter()
        .filter(|building| {
            view.definitions
                .building(building.kind)
                .is_some_and(|def| def.credits_per_ore.is_some())
        })
        .min_by_key(|building| {
            (
                octile_distance(building.footprint.origin, controller.start),
                building.id,
            )
        })?;
    let idle: Vec<SimId> = view
        .own_units
        .iter()
        .filter(|unit| unit.harvest == Some(HarvestState::Idle) && !unit.has_order)
        .filter(|unit| !controller.wave.contains(&unit.id))
        .map(|unit| unit.id)
        .collect();
    if idle.is_empty() {
        return None;
    }
    controller.next_harvest = view.tick.saturating_add(HARVEST_RETRY_TICKS);
    Some(Command::Harvest {
        units: idle,
        target: refinery.footprint.origin,
    })
}

// --- The army -------------------------------------------------------------------------------

/// Defence, gathering, and the attack wave.
fn command_army(view: &View<'_>, controller: &mut AiController) -> Vec<Command> {
    let mut commands = Vec::new();
    let armed: BTreeMap<SimId, &UnitSnap> = view
        .own_units
        .iter()
        .filter(|unit| {
            view.fights(unit)
                && (!view.ai.rules_difficulty
                    || view.definitions.domain(unit.kind) != Domain::Naval)
        })
        .map(|unit| (unit.id, *unit))
        .collect();
    // A living owned combat member frozen by a temporary ability keeps its mission,
    // but remains absent from active candidates and receives no commands until release.
    controller
        .wave
        .retain(|id| armed.contains_key(id) || view.frozen_army.contains(id));
    let home: Vec<&UnitSnap> = armed
        .values()
        .filter(|unit| !controller.wave.contains(&unit.id))
        .copied()
        .collect();

    // Defence: the visible enemy nearest to the start position among those within the radius.
    let radius = controller.difficulty.defence_radius;
    let near_base = |footprint: Footprint| {
        view.own_buildings
            .iter()
            .any(|building| building.footprint.distance(&footprint) <= radius)
    };
    let threat = view
        .enemy_units
        .iter()
        .map(|unit| (unit.id, cell_footprint(unit.mobile.cell)))
        .chain(
            view.enemy_buildings
                .iter()
                .map(|building| (building.id, building.footprint)),
        )
        .filter(|&(_, footprint)| near_base(footprint))
        .filter(|&(id, _)| {
            !view.ai.rules_difficulty
                || home
                    .iter()
                    .any(|unit| strategy::defence_reachable(view, unit, id))
        })
        .map(|(id, footprint)| (id, nearest_cell(footprint, controller.start)))
        .min_by_key(|&(id, cell)| (chebyshev(cell, controller.start), id));
    let mut defenders = BTreeSet::new();
    match threat {
        Some((target, cell)) => {
            if view.ai.rules_difficulty {
                for unit in &home {
                    if !strategy::defence_reachable(view, unit, target) {
                        continue;
                    }
                    defenders.insert(unit.id);
                    // Keep an existing chase intact, including its failure bound.
                    if !unit.attacking
                        && !matches!(unit.attack_order,
                        Some(crate::AttackOrder::Attack { target: current, .. }) if current == target)
                    {
                        commands.push(Command::Attack {
                            units: vec![unit.id],
                            target,
                        });
                    }
                }
            } else {
                let responders: Vec<SimId> = home
                    .iter()
                    .filter(|unit| !unit.attacking)
                    .map(|unit| unit.id)
                    .collect();
                if !responders.is_empty() {
                    commands.push(Command::AttackMove {
                        units: responders,
                        target: cell,
                    });
                }
            }
        }
        None => {
            let stray: Vec<SimId> = home
                .iter()
                .filter(|unit| {
                    unit.idle() && chebyshev(unit.mobile.cell, controller.gather) > GATHER_RADIUS
                })
                .map(|unit| unit.id)
                .collect();
            if !stray.is_empty() {
                commands.push(Command::AttackMove {
                    units: stray,
                    target: controller.gather,
                });
            }
        }
    }
    let home: Vec<_> = home
        .into_iter()
        .filter(|unit| !defenders.contains(&unit.id))
        .collect();

    // A wave reduced to less than half its size falls back to the gathering point and rejoins the
    // units at home, so that the next wave attacks with them at full size instead of feeding
    // units in one by one.
    if !controller.wave.is_empty()
        && controller.wave.len().saturating_mul(2) < controller.difficulty.wave_size as usize
        && !controller.fall_back_exempt
    {
        let back: Vec<SimId> = controller
            .wave
            .iter()
            .copied()
            .filter(|id| armed.contains_key(id))
            .collect();
        if back.is_empty() {
            // All surviving members are temporarily frozen. Keep this casualty
            // retreat pending until an active member can receive the ordinary order.
            return commands;
        }
        controller.wave.clear();
        controller.target = None;
        commands.push(Command::AttackMove {
            units: back,
            target: controller.gather,
        });
        return commands;
    }

    // The attack wave. Units that defend this turn do not join it.
    let mut joined: Vec<SimId> = Vec::new();
    let mut fresh = false;
    let ready = home.len() >= controller.difficulty.wave_size as usize
        || (!home.is_empty() && view.tick >= controller.patience_until);
    if (threat.is_none() || view.ai.rules_difficulty)
        && view.tick >= u64::from(controller.difficulty.first_wave)
        && view.tick >= controller.next_wave
        && ready
    {
        if let Some(rule) = strategy::difficulty(view, controller) {
            controller.next_wave = view
                .tick
                .saturating_add(u64::from(rule.team_delay.unsigned_abs()));
        }
        fresh = controller.wave.is_empty();
        controller.patience_until = view
            .tick
            .saturating_add(u64::from(controller.difficulty.wave_patience));
        // A wave sent after the patience ran out does not fall back: it is all there is.
        controller.fall_back_exempt = controller.wave.len().saturating_add(home.len())
            < controller.difficulty.wave_size as usize;
        joined = home.iter().map(|unit| unit.id).collect();
        controller.wave.extend(joined.iter().copied());
        // A reinforced wave tries the buildings it could not reach again.
        controller.failures.clear();
    }
    if controller.wave.is_empty() {
        controller.target = None;
        return commands;
    }
    let members: Vec<&UnitSnap> = controller
        .wave
        .iter()
        .filter_map(|id| armed.get(id).copied())
        .collect();
    if members.is_empty() {
        return commands;
    }
    let at_rest = members.iter().all(|unit| unit.idle()) && joined.is_empty();
    let centroid = centroid(&members, controller.start);
    // Large maps: a new wave first moves through a staging point beside the direct route.
    if controller.flank {
        if fresh {
            controller.waves_sent = controller.waves_sent.saturating_add(1);
            controller.target = choose_target(controller, centroid);
            let goal = target_cell(controller, centroid);
            controller.target = None;
            controller.staging = goal
                .and_then(|goal| staging_point(controller, centroid, goal))
                .map(|stage| (stage, view.tick.saturating_add(STAGING_TICKS)));
            if let Some((stage, _)) = controller.staging {
                commands.push(Command::AttackMove {
                    units: members.iter().map(|unit| unit.id).collect(),
                    target: stage,
                });
                return commands;
            }
        }
        if let Some((stage, until)) = controller.staging {
            // Reinforcements that just joined are idle at home; the wave rests when the rest are.
            let resting = members
                .iter()
                .filter(|unit| !joined.contains(&unit.id))
                .all(|unit| unit.idle());
            if resting || view.tick >= until || chebyshev(centroid, stage) <= STAGING_RADIUS {
                // Arrived (or stuck): on to the target, chosen afresh below.
                controller.staging = None;
                controller.target = None;
            } else {
                if !joined.is_empty() {
                    commands.push(Command::AttackMove {
                        units: joined,
                        target: stage,
                    });
                }
                return commands;
            }
        }
    }
    let valid = match controller.target {
        Some(WaveTarget::Building(id)) => {
            controller.known.contains_key(&id) && strategy::attack_job_eligible(controller, id)
        }
        // Scouting completes on actual observation. Slow aircraft or a crowded formation
        // need not all become idle at a cell that the player has already inspected.
        // A known building remains an attack objective until it is gone or unreachable.
        // Neutral legacy fixtures retain their declared original strategy (R91).
        Some(WaveTarget::Waypoint(index)) => {
            controller.waypoints.get(index).is_some_and(|waypoint| {
                !view.ai.rules_difficulty || !view.vision.is_visible(waypoint.cell)
            })
        }
        None => false,
    };
    let observed_scout = match controller.target {
        Some(WaveTarget::Waypoint(index)) => {
            controller.waypoints.get(index).is_some_and(|waypoint| {
                view.ai.rules_difficulty && view.vision.is_visible(waypoint.cell)
            })
        }
        _ => false,
    };
    if at_rest || !valid {
        if at_rest {
            match controller.target {
                Some(WaveTarget::Building(id)) if controller.known.contains_key(&id) => {
                    let failures = controller.failures.entry(id).or_insert(0);
                    *failures = failures.saturating_add(1);
                }
                Some(WaveTarget::Waypoint(index)) => {
                    if let Some(waypoint) = controller.waypoints.get_mut(index) {
                        // Reached or not reachable: it no longer has priority.
                        waypoint.seen = waypoint.seen.max(view.tick.saturating_add(1));
                    }
                }
                _ => {}
            }
        }
        controller.target = choose_target_with_sweep(controller, centroid, observed_scout);
        if let Some(cell) = target_cell(controller, centroid) {
            commands.push(Command::AttackMove {
                units: members.iter().map(|unit| unit.id).collect(),
                target: cell,
            });
        }
    } else if let Some(cell) = target_cell(controller, centroid) {
        // Rejoin comrades in combat after a blocked move. During travel, let arrived
        // members rest so the whole wave can finish its waypoint and choose another;
        // repeatedly ordering those members prevents that transition. Members already
        // within the gathering radius of the destination have arrived and stay at rest.
        let fighting = members.iter().any(|unit| unit.attacking);
        let reinforce: Vec<SimId> = members
            .iter()
            .filter(|unit| {
                joined.contains(&unit.id)
                    || (fighting
                        && unit.idle()
                        && chebyshev(unit.mobile.cell, cell) > GATHER_RADIUS)
            })
            .map(|unit| unit.id)
            .collect();
        if !reinforce.is_empty() {
            commands.push(Command::AttackMove {
                units: reinforce,
                target: cell,
            });
        }
    }
    commands
}

/// How close (Chebyshev, cells) a wave's centre must come to its staging point.
pub const STAGING_RADIUS: u32 = 6;

/// Ticks a wave may spend reaching its staging point (two minutes) before it attacks directly.
pub const STAGING_TICKS: u64 = 2400;

/// Waves whose target is nearer than this (octile distance, cells) attack directly.
pub const STAGING_MIN_DISTANCE: u32 = 40;

/// The staging point of a wave at `from` attacking `goal`: the waypoint nearest to the point
/// halfway along the route and a third of its length to one side, the side alternating with each
/// wave. Point-symmetric maps preserve orientation, so two players flanking on the same side take
/// mirrored routes on opposite sides of the map instead of meeting head-on. `None` for a near
/// target.
fn staging_point(controller: &AiController, from: CellPos, goal: CellPos) -> Option<CellPos> {
    if octile_distance(from, goal) < STAGING_MIN_DISTANCE {
        return None;
    }
    let (dx, dy) = (i64::from(goal.x - from.x), i64::from(goal.y - from.y));
    let side = if controller.waves_sent.is_multiple_of(2) {
        1
    } else {
        -1
    };
    let x = i64::from(from.x) + dx / 2 - side * dy / 3;
    let y = i64::from(from.y) + dy / 2 + side * dx / 3;
    let aim = CellPos::new(
        i32::try_from(x).unwrap_or(from.x),
        i32::try_from(y).unwrap_or(from.y),
    );
    controller
        .waypoints
        .iter()
        .map(|waypoint| waypoint.cell)
        .min_by_key(|&cell| (octile_distance(cell, aim), cell.y, cell.x))
}

/// The cell of `footprint` nearest to `cell` (Chebyshev distance, then row, then column).
fn nearest_cell(footprint: Footprint, cell: CellPos) -> CellPos {
    footprint
        .cells()
        .min_by_key(|&inside| (chebyshev(inside, cell), inside.y, inside.x))
        .unwrap_or(footprint.origin)
}

/// The mean cell of the units, or `fallback` without units.
fn centroid(units: &[&UnitSnap], fallback: CellPos) -> CellPos {
    if units.is_empty() {
        return fallback;
    }
    let count = units.len() as i64;
    let (x, y) = units.iter().fold((0_i64, 0_i64), |(x, y), unit| {
        (
            x.saturating_add(i64::from(unit.mobile.cell.x)),
            y.saturating_add(i64::from(unit.mobile.cell.y)),
        )
    });
    CellPos::new(
        i32::try_from(x / count).unwrap_or(fallback.x),
        i32::try_from(y / count).unwrap_or(fallback.y),
    )
}

/// The nearest remembered building that the wave has not failed to reach too often; else the
/// nearest enemy start position never seen; else begin a sweep near an enemy start. An
/// actually observed scouting leg continues to the nearest stale waypoint below.
fn choose_target(controller: &AiController, from: CellPos) -> Option<WaveTarget> {
    choose_target_with_sweep(controller, from, false)
}

fn choose_target_with_sweep(
    controller: &AiController,
    from: CellPos,
    continuing_sweep: bool,
) -> Option<WaveTarget> {
    let building = controller
        .known
        .iter()
        .filter(|(id, _)| controller.failures.get(id).copied().unwrap_or(0) < MAX_TARGET_FAILURES)
        .filter(|(id, _)| strategy::attack_job_eligible(controller, **id))
        .min_by_key(|(id, known)| (known.footprint.distance_to_cell(from), **id))
        .map(|(id, _)| WaveTarget::Building(*id));
    if building.is_some() {
        return building;
    }
    let unseen_start = controller
        .waypoints
        .iter()
        .enumerate()
        .filter(|(_, waypoint)| waypoint.priority && waypoint.seen == 0)
        .min_by_key(|(_, waypoint)| {
            (
                octile_distance(from, waypoint.cell),
                waypoint.cell.y,
                waypoint.cell.x,
            )
        })
        .map(|(index, _)| WaveTarget::Waypoint(index));
    if unseen_start.is_some() {
        return unseen_start;
    }
    // A sweep looks near the enemy starts first, where its remaining buildings most likely
    // stand, among the waypoints of the same era of last sight; a sweep in order of distance
    // from the wave alone starts at the wave's own base when the wave forms there, and wanders
    // the far corners of the map for a long time. (On the large maps since "Large maps with
    // environments"; on every map since RA1, whose slower units made the wandering of the
    // small maps cost a third of a match: docs/DECISIONS.md R14.)
    let near_enemy = |cell: CellPos| {
        controller
            .waypoints
            .iter()
            .filter(|waypoint| waypoint.priority)
            .map(|waypoint| octile_distance(cell, waypoint.cell))
            .min()
            .unwrap_or(0)
    };
    // R14's first sweep starts beside the enemy origin. Once that scouting leg is
    // actually observed, continue through nearby unseen ground instead of crossing the same
    // expanding ring repeatedly. A failed/unobserved land approach is not an observation:
    // a later aircraft must still search the original enemy area before wandering elsewhere.
    controller
        .waypoints
        .iter()
        .enumerate()
        .min_by_key(|(_, waypoint)| {
            (
                waypoint.seen / SWEEP_ERA_TICKS,
                if continuing_sweep {
                    0
                } else {
                    near_enemy(waypoint.cell)
                },
                octile_distance(from, waypoint.cell),
                waypoint.cell.y,
                waypoint.cell.x,
            )
        })
        .map(|(index, _)| WaveTarget::Waypoint(index))
}

/// Waypoints last seen within the same span of ticks (five minutes) count as equally stale when
/// a wave sweeps for buildings it does not know.
pub const SWEEP_ERA_TICKS: u64 = 6000;

/// The cell the wave attack-moves to for its target.
fn target_cell(controller: &AiController, from: CellPos) -> Option<CellPos> {
    match controller.target? {
        WaveTarget::Building(id) => controller
            .known
            .get(&id)
            .map(|known| nearest_cell(known.footprint, from)),
        WaveTarget::Waypoint(index) => controller
            .waypoints
            .get(index)
            .map(|waypoint| waypoint.cell),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::map::Cell;

    fn ra8_controller() -> AiController {
        let grid = Grid::from_cells(
            32,
            32,
            vec![
                Cell {
                    passable: true,
                    water: false,
                    buildable: true,
                    ore: 0
                };
                32 * 32
            ],
        )
        .expect("grid");
        AiController::new(
            &grid,
            &Definitions::default(),
            AiPlan {
                policy: Default::default(),
                rules_difficulty: false,
                defences: Vec::new(),
                support: Vec::new(),
                tier_reserves: [0; 3],
                advanced_tech_level: 0,
                build_order: Vec::new(),
                power: BuildingKind(0),
                harvester: UnitKind(0),
                army: Vec::new(),
            },
            Difficulty {
                id: "test".into(),
                think_interval: 10,
                harvesters_per_refinery: 0,
                max_army: 4,
                wave_size: 2,
                first_wave: 0,
                wave_patience: 100,
                defence_radius: 4,
            },
            CellPos::new(4, 4),
            Vec::new(),
            None,
        )
        .expect("controller")
    }

    #[test]
    fn ra8_every_supported_cadence_and_offset_retains_a_nonstarving_last_stand_slot() {
        let mut controller = ra8_controller();
        for interval in 1..=crate::MAX_THINK_INTERVAL {
            controller.difficulty.think_interval = interval;
            for player in 0..16 {
                let player = PlayerId(player);
                let ordinary = 4096 + u64::from(player.0);
                controller.last_stand = false;
                assert_eq!(
                    due(player, &controller, ordinary),
                    ordinary % u64::from(interval) == u64::from(player.0) % u64::from(interval)
                );
                controller.last_stand = true;
                let slot = 4096 + u64::from(player.0) * LAST_STAND_TURN;
                assert!(due(player, &controller, slot), "{interval}, {player:?}");
                assert_eq!((slot / LAST_STAND_TURN) % 16, u64::from(player.0));
            }
        }
        // Simultaneous helpless houses with mixed slow cadences never share a sale slot.
        for tick in (4096..8192).step_by(LAST_STAND_TURN as usize) {
            let sales: Vec<_> = (0..16)
                .filter(|&player| {
                    controller.difficulty.think_interval =
                        [10, 20, 40, 512, 1024, 1200][player as usize % 6];
                    due(PlayerId(player), &controller, tick)
                        && (tick / LAST_STAND_TURN) % 16 == u64::from(player)
                })
                .collect();
            assert_eq!(sales.len(), 1);
        }
        assert!(!AiPlayers::default().thinks_at(0, &MatchState::default()));
    }

    #[test]
    fn ra8_service_recovery_does_not_admit_unavailable_or_foreign_actors_to_the_army() {
        let definitions = crate::load_ruleset(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data"),
        )
        .unwrap();
        let grid = Grid::from_cells(
            32,
            32,
            vec![
                Cell {
                    passable: true,
                    water: false,
                    buildable: true,
                    ore: 0,
                };
                32 * 32
            ],
        )
        .unwrap();
        let mut players = Players::default();
        players.add(PlayerId(0), 0).unwrap();
        let state = players.get(PlayerId(0)).unwrap();
        let plan = ra8_controller().plan;
        let unit = UnitSnap {
            hp: 200,
            id: SimId(1),
            owner: PlayerId(0),
            kind: definitions.unit_kind("grizzly").unwrap(),
            mobile: Mobile {
                speed: 16,
                cell: CellPos::new(10, 10),
                step: None,
            },
            has_order: false,
            harvest: None,
            armed: true,
            attacking: false,
            attack_order: None,
            combat_target: None,
            move_progress: None,
            sight: Some(4),
        };
        let building = BuildingSnap {
            economy: true,
            hp: 1000,
            id: SimId(2),
            owner: PlayerId(0),
            kind: definitions.building_kind("service_depot").unwrap(),
            footprint: Footprint {
                origin: CellPos::new(11, 10),
                width: 3,
                height: 2,
            },
            producer: None,
            sight: Some(4),
            pending: Vec::new(),
        };
        let mut special = crate::SpecialState::default();
        special.docked.insert(unit.id, building.id);
        let check =
            |special: &crate::SpecialState, unit: &UnitSnap, building: &BuildingSnap, expected| {
                let units = std::slice::from_ref(unit);
                let buildings = std::slice::from_ref(building);
                let view = View::new(
                    PlayerId(0),
                    0,
                    &players,
                    state,
                    &grid,
                    &definitions,
                    &plan,
                    false,
                    units,
                    buildings,
                    special,
                );
                assert!(
                    view.own_units.is_empty(),
                    "service occupants stay outside ordinary army selection"
                );
                assert_eq!(view.recoverable_service.len(), expected);
            };
        check(&special, &unit, &building, 1);
        for change in [
            |s: &mut crate::SpecialState| {
                s.deployed.insert(SimId(1), 0);
            },
            |s: &mut crate::SpecialState| {
                s.disabled.insert(SimId(1), u64::MAX);
            },
            |s: &mut crate::SpecialState| {
                s.rearming.insert(SimId(1), 100);
            },
            |s: &mut crate::SpecialState| {
                s.interactions.insert(SimId(1), SimId(2));
            },
            |s: &mut crate::SpecialState| {
                s.spawned.insert(SimId(1), SimId(2));
            },
            |s: &mut crate::SpecialState| {
                s.erasures.insert(
                    SimId(1),
                    crate::special::infantry::Erasure {
                        remaining: 100,
                        beams: BTreeMap::new(),
                    },
                );
            },
            |s: &mut crate::SpecialState| {
                s.strategic.flights.insert(
                    SimId(1),
                    crate::special::superweapons::Flight {
                        owner: PlayerId(0),
                        target: CellPos::new(20, 20),
                        infantry: UnitKind(0),
                        count: 1,
                    },
                );
            },
            |s: &mut crate::SpecialState| {
                s.parasites.insert(
                    SimId(1),
                    crate::special::vehicles::Parasite {
                        passenger: crate::special::Passenger {
                            id: SimId(3),
                            kind: UnitKind(0),
                            hp: 10,
                            harvester: None,
                            experience: None,
                            bay: None,
                        },
                        owner: PlayerId(1),
                        weapon: crate::WeaponKind(0),
                        due: 100,
                        squid: false,
                        escape: None,
                    },
                );
            },
        ] {
            let mut unavailable = special.clone();
            change(&mut unavailable);
            check(&unavailable, &unit, &building, 0);
        }
        for change in [
            |b: &mut BuildingSnap| {
                b.owner = PlayerId(1);
            },
            |b: &mut BuildingSnap| {
                b.hp = 0;
            },
            |b: &mut BuildingSnap| {
                b.economy = false;
            },
        ] {
            let mut invalid = BuildingSnap {
                pending: Vec::new(),
                ..building
            };
            change(&mut invalid);
            check(&special, &unit, &invalid, 0);
        }
    }

    #[test]
    fn ra8_aa_job_progress_survives_reinforcement_and_preserves_slow_approaches() {
        let mut progress = AirScreenProgress {
            remaining: 300,
            distance: 30,
            hp: 900,
            last_progress: 0,
        };
        // New participants, repairs and restarted longer routes are not progress.
        for tick in 1..=3000 {
            progress.advance(400, 35, 900 + (tick % 2) as i32, tick);
        }
        assert_eq!(progress.last_progress, 0);
        // The expired job remains expired on the next decision with identical facts.
        progress.advance(300, 30, 900, 3020);
        assert_eq!(progress.last_progress, 0);
        // An actual improving detour can be slow and temporarily farther away.
        progress.advance(299, 31, 900, 3040);
        assert_eq!(progress.last_progress, 3040);
        progress.advance(299, 29, 900, 5000);
        assert_eq!(progress.last_progress, 5000);
        progress.advance(299, 29, 899, 7000);
        assert_eq!(progress.last_progress, 7000);
        assert_eq!(
            (progress.remaining, progress.distance, progress.hp),
            (299, 29, 899)
        );
    }

    #[test]
    fn ra8_each_aa_job_progress_field_changes_the_value_hash() {
        let mut base = ra8_controller();
        base.air_screens.insert(
            SimId(1),
            AirScreenProgress {
                remaining: 20,
                distance: 3,
                hp: 900,
                last_progress: 10,
            },
        );
        let hash = |c: &AiController| {
            let mut h = Fnv1a64::new();
            c.hash_into(&mut h);
            h.finish()
        };
        let changes: &[fn(&mut AiController)] = &[
            |c| c.air_screens.clear(),
            |c| {
                c.air_screens.get_mut(&SimId(1)).unwrap().remaining = 19;
            },
            |c| {
                c.air_screens.get_mut(&SimId(1)).unwrap().distance = 2;
            },
            |c| {
                c.air_screens.get_mut(&SimId(1)).unwrap().hp = 899;
            },
            |c| {
                c.air_screens.get_mut(&SimId(1)).unwrap().last_progress = 11;
            },
        ];
        for change in changes {
            let mut changed = base.clone();
            change(&mut changed);
            assert_ne!(hash(&base), hash(&changed));
        }
    }

    #[test]
    fn ra8_all_added_controller_memory_changes_the_value_hash() {
        let base = ra8_controller();
        let hash = |c: &AiController| {
            let mut h = Fnv1a64::new();
            c.hash_into(&mut h);
            h.finish()
        };
        let baseline = hash(&base);
        let changes: &[fn(&mut AiController)] = &[
            |c| c.next_wave = 1,
            |c| c.seen_air = true,
            |c| c.unseen_loss = true,
            |c| {
                c.observed_units.insert(SimId(1), CellPos::new(2, 3));
            },
            |c| {
                c.reserved.insert(SimId(1));
            },
            |c| c.naval_goal = Some(CellPos::new(1, 2)),
            |c| c.naval_site = Some(CellPos::new(1, 2)),
            |c| c.next_naval_scan = 1,
            |c| {
                c.known.insert(
                    SimId(1),
                    KnownBuilding {
                        owner: PlayerId(1),
                        kind: BuildingKind(1),
                        footprint: cell_footprint(CellPos::new(2, 3)),
                    },
                );
            },
            |c| c.plan.rules_difficulty = true,
            |c| c.plan.defences.push(BuildingKind(1)),
            |c| c.plan.support.push(UnitKind(1)),
            |c| c.plan.tier_reserves[0] = 1,
            |c| c.plan.tier_reserves[1] = 1,
            |c| c.plan.tier_reserves[2] = 1,
            |c| c.plan.advanced_tech_level = 1,
            |c| c.plan.policy.garrison_quota = 1,
            |c| c.plan.policy.garrison_radius = 1,
            |c| c.plan.policy.depot_radius = 1,
            |c| c.plan.policy.naval_rescan = 1,
            |c| c.plan.policy.naval_detector_quota = 1,
            |c| c.plan.policy.artillery_min_range = 1,
            |c| c.plan.policy.artillery_standoff = 1,
            |c| c.plan.policy.paradrop_offset = 1,
            |c| c.plan.policy.chrono_min_offset = 1,
            |c| c.plan.policy.chrono_max_offset = 1,
        ];
        for (index, change) in changes.iter().enumerate() {
            let mut changed = base.clone();
            change(&mut changed);
            assert_ne!(hash(&changed), baseline, "field {index}");
        }
        let mut a = base;
        a.known.insert(
            SimId(1),
            KnownBuilding {
                owner: PlayerId(1),
                kind: BuildingKind(1),
                footprint: cell_footprint(CellPos::new(2, 3)),
            },
        );
        let before = hash(&a);
        a.known.get_mut(&SimId(1)).expect("known").kind = BuildingKind(2);
        assert_ne!(before, hash(&a));
    }

    #[test]
    fn ra5_bridge_memory_hash_covers_hut_identity_location_and_observed_collapse() {
        let mut controller = AiController {
            plan: AiPlan {
                policy: Default::default(),
                rules_difficulty: false,
                defences: Vec::new(),
                support: Vec::new(),
                tier_reserves: [0; 3],
                advanced_tech_level: 0,
                build_order: Vec::new(),
                power: BuildingKind(0),
                harvester: UnitKind(0),
                army: Vec::new(),
            },
            difficulty: Difficulty {
                id: "test".into(),
                think_interval: 10,
                harvesters_per_refinery: 1,
                max_army: 4,
                wave_size: 2,
                first_wave: 0,
                wave_patience: 100,
                defence_radius: 4,
            },
            start: CellPos::new(1, 1),
            gather: CellPos::new(2, 2),
            waypoints: Vec::new(),
            known: BTreeMap::new(),
            repair_huts: BTreeMap::new(),
            broken_bridges: BTreeSet::new(),
            wave: BTreeSet::new(),
            target: None,
            failures: BTreeMap::new(),
            next_army: 0,
            army_cursors: BTreeMap::new(),
            patience_until: 0,
            next_harvest: 0,
            fall_back_exempt: false,
            liquidating: false,
            issued: 0,
            flank: false,
            staging: None,
            waves_sent: 0,
            stalled: (0, 0),
            last_stand: false,
            next_wave: 0,
            seen_air: false,
            unseen_loss: false,
            observed_units: BTreeMap::new(),
            reserved: BTreeSet::new(),
            naval_goal: None,
            naval_site: None,
            next_naval_scan: 0,
            air_screens: BTreeMap::new(),
            building_jobs: BTreeMap::new(),
        };
        let hash = |c: &AiController| {
            let mut h = Fnv1a64::new();
            c.hash_into(&mut h);
            h.finish()
        };
        let empty = hash(&controller);
        controller
            .repair_huts
            .insert(SimId(5), (SimId(9), CellPos::new(3, 4)));
        let known = hash(&controller);
        assert_ne!(empty, known);
        controller.broken_bridges.insert(SimId(9));
        let broken = hash(&controller);
        assert_ne!(known, broken);
        for (hut, bridge, cell) in [
            (6, 9, CellPos::new(3, 4)),
            (5, 10, CellPos::new(3, 4)),
            (5, 9, CellPos::new(4, 4)),
        ] {
            controller.repair_huts.clear();
            controller
                .repair_huts
                .insert(SimId(hut), (SimId(bridge), cell));
            assert_ne!(broken, hash(&controller));
        }
        controller.repair_huts.clear();
        controller.broken_bridges.clear();
        assert_eq!(empty, hash(&controller));
    }

    /// Regression (RA3, docs/DECISIONS.md R44; found by `m6_production_matches.rs`: Thousand
    /// Lakes, Iraq against Korea, both brutal, tick 13,622). A computer player placed a
    /// refinery on cells that only a Chrono Miner standing beside them lit; the miner, full,
    /// teleported to its refinery in the tick of the command, and the placement was rejected
    /// as not visible without any loss. A unit that teleports lights nothing a placement may
    /// rely on; a unit that walks, and a building, do as before.
    #[test]
    fn a_cell_lit_only_by_a_unit_that_teleports_is_not_counted_on() {
        // The legacy ruleset has the Chrono Miner and the Chrono Legionnaire of M9.
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data/legacy");
        let definitions = crate::load_match_definitions(
            dir.join("units.ron"),
            dir.join("economy.ron"),
            dir.join("combat.ron"),
        )
        .and_then(|definitions| definitions.with_arsenal(dir.join("arsenal.ron")))
        .unwrap_or_else(|error| panic!("{error}"));
        let kind = |id: &str| definitions.unit_kind(id).unwrap_or_else(|| panic!("{id}"));
        let role = |id: &str| {
            definitions
                .metadata(ProductionItem::Unit(kind(id)))
                .map(|entry| entry.role)
        };
        assert_eq!(role("chrono_miner"), Some(Role::ChronoMiner));
        assert_eq!(role("chrono_legionnaire"), Some(Role::Chrono));
        let cell = Cell {
            passable: true,
            water: false,
            buildable: true,
            ore: 0,
        };
        let grid = Grid::from_cells(32, 32, vec![cell; 32 * 32]).expect("a grid");
        let player = PlayerId(0);
        let mut players = Players::default();
        players.add(player, 0).expect("a new player");
        let state = players.get(player).expect("the player");
        let plan = AiPlan {
            policy: Default::default(),
            rules_difficulty: false,
            defences: Vec::new(),
            support: Vec::new(),
            tier_reserves: [0; 3],
            advanced_tech_level: 0,
            build_order: Vec::new(),
            power: BuildingKind(0),
            harvester: kind("harvester"),
            army: Vec::new(),
        };
        // A unit of the type `id` standing at (10, 10) with a sight of 4 cells.
        let standing = |id: &str| UnitSnap {
            hp: 100,
            id: SimId(1),
            owner: player,
            kind: kind(id),
            mobile: Mobile {
                speed: 16,
                cell: CellPos::new(10, 10),
                step: None,
            },
            has_order: false,
            harvest: None,
            armed: false,
            attacking: false,
            attack_order: None,
            combat_target: None,
            move_progress: None,
            sight: Some(4),
        };
        let yard = BuildingSnap {
            economy: true,
            hp: 100,
            id: SimId(2),
            owner: player,
            kind: BuildingKind(0),
            footprint: Footprint {
                origin: CellPos::new(20, 20),
                width: 2,
                height: 2,
            },
            producer: None,
            sight: Some(3),
            pending: Vec::new(),
        };
        let lit = |id: &str, cell: CellPos| {
            let units = [standing(id)];
            let buildings = [BuildingSnap {
                hp: 100,
                pending: Vec::new(),
                ..yard
            }];
            View::new(
                player,
                0,
                &players,
                state,
                &grid,
                &definitions,
                &plan,
                false,
                &units,
                &buildings,
                &crate::SpecialState::default(),
            )
            .stays_visible(cell)
        };
        let beside_the_unit = CellPos::new(12, 12);
        // A unit that walks stays where it stands for the next tick.
        assert!(lit("harvester", beside_the_unit));
        assert!(lit("rifleman", beside_the_unit));
        // One that teleports may be anywhere.
        assert!(!lit("chrono_miner", beside_the_unit));
        assert!(!lit("chrono_legionnaire", beside_the_unit));
        // A building lights its surroundings whatever stands there, and nothing lights a cell
        // out of every sight.
        for id in ["harvester", "chrono_miner"] {
            assert!(lit(id, CellPos::new(23, 21)), "{id}");
            assert!(!lit(id, CellPos::new(16, 16)), "{id}");
        }
    }

    /// A blocked member must rejoin an active wave without cancelling the other members'
    /// moves or attacks. Previously only new recruits were ordered until every member rested.
    #[test]
    fn an_idle_wave_member_rejoins_while_its_comrades_keep_fighting() {
        let grid = Grid::from_cells(
            32,
            32,
            vec![
                Cell {
                    passable: true,
                    water: false,
                    buildable: true,
                    ore: 0,
                };
                32 * 32
            ],
        )
        .expect("grid");
        let definitions = Definitions::default();
        let player = PlayerId(0);
        let mut players = Players::default();
        players.add(player, 0).expect("player");
        let state = players.get(player).expect("player");
        let plan = AiPlan {
            policy: Default::default(),
            rules_difficulty: false,
            defences: Vec::new(),
            support: Vec::new(),
            tier_reserves: [0; 3],
            advanced_tech_level: 0,
            build_order: Vec::new(),
            power: BuildingKind(0),
            harvester: UnitKind(0),
            army: Vec::new(),
        };
        let difficulty = Difficulty {
            id: "test".into(),
            think_interval: 10,
            harvesters_per_refinery: 1,
            max_army: 4,
            wave_size: 2,
            first_wave: 0,
            wave_patience: 100,
            defence_radius: 4,
        };
        let mut controller = AiController::new(
            &grid,
            &definitions,
            plan.clone(),
            difficulty,
            CellPos::new(4, 4),
            vec![CellPos::new(28, 28)],
            None,
        )
        .expect("controller");
        controller.wave = [SimId(1), SimId(2)].into_iter().collect();
        controller.target = Some(WaveTarget::Waypoint(0));
        let mut units = [false, true].map(|attacking| UnitSnap {
            hp: 100,
            id: SimId(if attacking { 2 } else { 1 }),
            owner: player,
            kind: UnitKind(0),
            mobile: Mobile {
                speed: 16,
                cell: CellPos::new(10, 10),
                step: None,
            },
            has_order: false,
            harvest: None,
            armed: true,
            attacking,
            attack_order: None,
            combat_target: None,
            move_progress: None,
            sight: Some(4),
        });
        let view = View::new(
            player,
            1000,
            &players,
            state,
            &grid,
            &definitions,
            &plan,
            false,
            &units,
            &[],
            &crate::SpecialState::default(),
        );
        let commands = command_army(&view, &mut controller);
        assert_eq!(
            commands,
            vec![Command::AttackMove {
                units: vec![SimId(1)],
                target: CellPos::new(28, 28)
            }]
        );
        // A comrade still travelling must not make arrived members restart forever:
        // they need to rest together to retire the current search waypoint.
        units[1].attacking = false;
        units[1].has_order = true;
        let view = View::new(
            player,
            1000,
            &players,
            state,
            &grid,
            &definitions,
            &plan,
            false,
            &units,
            &[],
            &crate::SpecialState::default(),
        );
        assert!(command_army(&view, &mut controller).is_empty());
        units[1].attacking = true;
        units[0].mobile.cell = CellPos::new(27, 28);
        let view = View::new(
            player,
            1000,
            &players,
            state,
            &grid,
            &definitions,
            &plan,
            false,
            &units,
            &[],
            &crate::SpecialState::default(),
        );
        assert!(command_army(&view, &mut controller).is_empty());
    }

    /// RA1 regression (docs/DECISIONS.md R14, R56; found by `m14_duels -- full`: Crossroads,
    /// Libya against Iraq, both easy). A wave that knows no building and has seen every enemy
    /// start sweeps the map for what is left. It took the waypoints in the order of their
    /// distance from itself, so a wave formed at its own base wandered its own half of the map
    /// for 13,000 ticks before it looked beside the loser's start, where the last buildings
    /// stood. A sweep starts at the waypoints nearest to an enemy start, on every map. (The
    /// end to end test of that match in `crates/game/tests/m6_production_matches.rs` could no
    /// longer fail by its limit once the limit was doubled in RA2: this test fails without
    /// the rule, whatever the limit.)
    #[test]
    fn a_sweep_starts_at_the_waypoint_nearest_to_an_enemy_start() {
        let cell = Cell {
            passable: true,
            water: false,
            buildable: true,
            ore: 0,
        };
        // A small map (the rule held on large maps before RA1).
        let grid = Grid::from_cells(64, 64, vec![cell; 64 * 64]).expect("a grid");
        assert!(grid.width() + grid.height() <= ARMY_SCALE_SPAN);
        let plan = AiPlan {
            policy: Default::default(),
            rules_difficulty: false,
            defences: Vec::new(),
            support: Vec::new(),
            tier_reserves: [0; 3],
            advanced_tech_level: 0,
            build_order: Vec::new(),
            power: BuildingKind(0),
            harvester: UnitKind(0),
            army: Vec::new(),
        };
        let difficulty = Difficulty {
            id: "test".into(),
            think_interval: 10,
            harvesters_per_refinery: 1,
            max_army: 4,
            wave_size: 2,
            first_wave: 0,
            wave_patience: 100,
            defence_radius: 4,
        };
        let own = CellPos::new(6, 6);
        let enemy = CellPos::new(56, 56);
        let mut controller = AiController::new(
            &grid,
            &Definitions::default(),
            plan,
            difficulty,
            own,
            vec![enemy],
            None,
        )
        .expect("a controller");
        // It knows no building, and every waypoint, the enemy's start among them, was seen
        // in the same era: beside its own base, in the middle, in a far corner, beside the
        // enemy's start, and the start itself.
        let waypoint = |x: i32, y: i32, priority: bool| Waypoint {
            cell: CellPos::new(x, y),
            seen: 100,
            priority,
        };
        controller.known.clear();
        controller.waypoints = vec![
            waypoint(10, 10, false),
            waypoint(32, 32, false),
            waypoint(8, 56, false),
            waypoint(50, 52, false),
            waypoint(enemy.x, enemy.y, true),
        ];
        // From its own base (where the wave forms) the sweep goes to the enemy's start first,
        assert_eq!(
            choose_target(&controller, own),
            Some(WaveTarget::Waypoint(4)),
            "not the waypoint nearest to the wave"
        );
        // then, the start seen in a later era, to the waypoint beside it, not to those near
        // the wave,
        controller.waypoints[4].seen = 100 + SWEEP_ERA_TICKS;
        assert_eq!(
            choose_target(&controller, own),
            Some(WaveTarget::Waypoint(3))
        );
        // and so on outwards from the enemy's start.
        controller.waypoints[3].seen = 100 + SWEEP_ERA_TICKS;
        assert_eq!(
            choose_target(&controller, own),
            Some(WaveTarget::Waypoint(1))
        );
        // A start never seen comes before any sweep, and a known building before that.
        controller.waypoints[4].seen = 0;
        assert_eq!(
            choose_target(&controller, own),
            Some(WaveTarget::Waypoint(4))
        );
    }

    /// RA4 regression (docs/DECISIONS.md R53). A player with no unit that fights and no
    /// harvester at work, whose credits, units, buildings and queues stand still, sells its
    /// buildings after [`LAST_STAND_PATIENCE`] ticks, the dearest first, one a turn and only in
    /// the turn of its player id. A queue it cannot pay for carries a fraction of a step from
    /// tick to tick: that is no change (the defect: with it in the picture a player whose
    /// barracks had a soldier queued never made its last stand, and a single Conscript took
    /// 40,000 ticks to wear down its four idle, self healing miners).
    #[test]
    fn a_player_that_can_neither_fight_nor_earn_makes_its_last_stand() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data/legacy");
        let definitions = crate::load_match_definitions(
            dir.join("units.ron"),
            dir.join("economy.ron"),
            dir.join("combat.ron"),
        )
        .and_then(|definitions| definitions.with_arsenal(dir.join("arsenal.ron")))
        .unwrap_or_else(|error| panic!("{error}"));
        let unit_kind = |id: &str| definitions.unit_kind(id).unwrap_or_else(|| panic!("{id}"));
        let building_kind = |id: &str| {
            definitions
                .building_kind(id)
                .unwrap_or_else(|| panic!("{id}"))
        };
        let cell = Cell {
            passable: true,
            water: false,
            buildable: true,
            ore: 0,
        };
        let grid = Grid::from_cells(32, 32, vec![cell; 32 * 32]).expect("a grid");
        let plan = AiPlan {
            policy: Default::default(),
            rules_difficulty: false,
            defences: Vec::new(),
            support: Vec::new(),
            tier_reserves: [0; 3],
            advanced_tech_level: 0,
            build_order: Vec::new(),
            power: BuildingKind(0),
            harvester: unit_kind("harvester"),
            army: Vec::new(),
        };
        let difficulty = Difficulty {
            id: "test".into(),
            think_interval: 10,
            harvesters_per_refinery: 1,
            max_army: 4,
            wave_size: 2,
            first_wave: 0,
            wave_patience: 100,
            defence_radius: 4,
        };
        let controller = |player: PlayerId| {
            AiController::new(
                &grid,
                &definitions,
                plan.clone(),
                difficulty.clone(),
                CellPos::new(5 + i32::from(player.0), 5),
                vec![CellPos::new(25, 25)],
                None,
            )
            .expect("a controller")
        };
        // The yard is worth more than the barracks when sold.
        let (yard, barracks) = (
            building_kind("construction_yard"),
            building_kind("barracks"),
        );
        assert!(definitions.refund(yard) > definitions.refund(barracks));
        let building = |id: u32, kind: BuildingKind, x: i32| BuildingSnap {
            economy: true,
            hp: 100,
            id: SimId(id),
            owner: PlayerId(0),
            kind,
            footprint: Footprint {
                origin: CellPos::new(x, 20),
                width: 2,
                height: 2,
            },
            producer: None,
            sight: Some(3),
            pending: Vec::new(),
        };
        // An idle harvester (no ore) and, in the last case, a rifleman.
        let unit = |id: u32, kind: &str, armed: bool| UnitSnap {
            hp: 100,
            id: SimId(id),
            owner: PlayerId(0),
            kind: unit_kind(kind),
            mobile: Mobile {
                speed: 16,
                cell: CellPos::new(10 + id as i32, 10),
                step: None,
            },
            has_order: false,
            harvest: (!armed).then_some(HarvestState::Idle),
            armed,
            attacking: false,
            attack_order: None,
            combat_target: None,
            move_progress: None,
            sight: Some(4),
        };
        // One turn of player 0 at `tick`, with `credits`, a soldier queued whose step of
        // progress stands at `fraction`, and the units `units`.
        let turn = |controller: &mut AiController,
                    tick: u64,
                    credits: i64,
                    fraction: u32,
                    units: &[UnitSnap]| {
            let player = PlayerId(0);
            let mut players = Players::default();
            players.add(player, credits).expect("a new player");
            let rifleman = ProductionItem::Unit(unit_kind("rifleman"));
            let key = definitions.queue_of(rifleman).expect("a queue");
            let mut queued = crate::economy::QueueItem::new(rifleman);
            queued.fraction = fraction;
            players
                .get_mut(player)
                .expect("the player")
                .queues
                .insert(key, vec![queued]);
            let state = players.get(player).expect("the player");
            let buildings = [building(2, yard, 20), building(3, barracks, 24)];
            let view = View::new(
                player,
                tick,
                &players,
                state,
                &grid,
                &definitions,
                &plan,
                false,
                units,
                &buildings,
                &crate::SpecialState::default(),
            );
            last_stand(&view, controller)
        };
        let idle = [unit(1, "harvester", false)];
        let sell = |id: u32| {
            Some(Command::SellBuilding {
                building: SimId(id),
            })
        };
        let mut own = controller(PlayerId(0));
        // It waits out its patience, whatever the fraction of the queue does.
        assert_eq!(turn(&mut own, 4_000, 0, 0, &idle), None);
        assert_eq!(turn(&mut own, 4_600, 0, 400, &idle), None);
        assert_eq!(
            turn(&mut own, 4_000 + LAST_STAND_PATIENCE - 1, 0, 800, &idle),
            None
        );
        // Then it sells in the turn of its id (ticks 8192 to 8447 for player 0), the yard first.
        assert_eq!(
            turn(&mut own, 4_000 + LAST_STAND_PATIENCE, 0, 200, &idle),
            None
        );
        assert_eq!(turn(&mut own, 8_191, 0, 200, &idle), None);
        assert_eq!(turn(&mut own, 8_192, 0, 600, &idle), sell(2));
        assert_eq!(turn(&mut own, 8_447, 0, 600, &idle), sell(2));
        assert_eq!(turn(&mut own, 8_448, 0, 600, &idle), None);
        // A change of what it has starts the patience anew: here its credits.
        let mut own = controller(PlayerId(0));
        assert_eq!(turn(&mut own, 4_000, 0, 0, &idle), None);
        assert_eq!(turn(&mut own, 5_000, 70, 0, &idle), None);
        assert_eq!(turn(&mut own, 8_192, 70, 0, &idle), sell(2));
        // Credits it does not spend are no change either: a player whose finished building
        // has no site is as stuck as one that can pay for nothing, and what counts is that
        // nothing it has moves (advisory review 2, sim, suspected item 2; R56).
        let mut own = controller(PlayerId(0));
        assert_eq!(turn(&mut own, 4_000, 5_000, 0, &idle), None);
        assert_eq!(turn(&mut own, 8_192, 5_000, 0, &idle), sell(2));
        let mut own = controller(PlayerId(0));
        assert_eq!(turn(&mut own, 7_500, 0, 0, &idle), None);
        assert_eq!(
            turn(&mut own, 8_192, 0, 0, &idle),
            None,
            "692 ticks are not enough"
        );
        // With a unit that fights there is no last stand, and one under way ends.
        let armed = [unit(1, "harvester", false), unit(4, "rifleman", true)];
        let mut own = controller(PlayerId(0));
        assert_eq!(turn(&mut own, 0, 0, 0, &armed), None);
        assert_eq!(turn(&mut own, 8_192, 0, 0, &armed), None);
        assert_eq!(
            turn(&mut own, 12_288, 0, 0, &idle),
            None,
            "the patience starts now"
        );
        assert_eq!(turn(&mut own, 16_384, 0, 0, &idle), sell(2));
        assert_eq!(turn(&mut own, 16_390, 0, 0, &armed), None);
        assert_eq!(turn(&mut own, 16_400, 0, 0, &idle), None);
        // Two players never sell in the same tick: the turns of their ids do not overlap.
        for tick in 0..16 * LAST_STAND_TURN {
            let turns = (0..10u64)
                .filter(|player| (tick / LAST_STAND_TURN) % 16 == player % 16)
                .count();
            assert!(turns <= 1);
        }
    }

    /// RA4 (docs/DECISIONS.md R51, R50): the computer player keeps its honest vision. An enemy
    /// unit that is hidden from it (a submerged unit without a detector near) is none of the
    /// enemies it sees, though it sees the cell; and a missile or aircraft that one of its
    /// units launched is none of its units: it is not commanded, not counted as an armed unit,
    /// and its sight is not counted on (it is gone when it has done its work).
    #[test]
    fn a_hidden_enemy_is_not_seen_and_a_launched_unit_is_not_the_players_to_command() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data/legacy");
        let definitions = crate::load_match_definitions(
            dir.join("units.ron"),
            dir.join("economy.ron"),
            dir.join("combat.ron"),
        )
        .and_then(|definitions| definitions.with_arsenal(dir.join("arsenal.ron")))
        .unwrap_or_else(|error| panic!("{error}"));
        let kind = |id: &str| definitions.unit_kind(id).unwrap_or_else(|| panic!("{id}"));
        let cell = Cell {
            passable: true,
            water: false,
            buildable: true,
            ore: 0,
        };
        let grid = Grid::from_cells(32, 32, vec![cell; 32 * 32]).expect("a grid");
        let (player, enemy) = (PlayerId(0), PlayerId(1));
        let mut players = Players::default();
        players.add(player, 0).expect("a new player");
        players.add(enemy, 0).expect("a new player");
        // The player sees the cells around (10, 10).
        players
            .get_mut(player)
            .expect("the player")
            .vision
            .reveal_for_test(&grid, CellPos::new(10, 10), 6);
        let state = players.get(player).expect("the player");
        let plan = AiPlan {
            policy: Default::default(),
            rules_difficulty: false,
            defences: Vec::new(),
            support: Vec::new(),
            tier_reserves: [0; 3],
            advanced_tech_level: 0,
            build_order: Vec::new(),
            power: BuildingKind(0),
            harvester: kind("harvester"),
            army: Vec::new(),
        };
        let unit = |id: u32, owner: PlayerId, x: i32| UnitSnap {
            hp: 100,
            id: SimId(id),
            owner,
            kind: kind("rifleman"),
            mobile: Mobile {
                speed: 16,
                cell: CellPos::new(x, 10),
                step: None,
            },
            has_order: false,
            harvest: None,
            armed: true,
            attacking: false,
            attack_order: None,
            combat_target: None,
            move_progress: None,
            sight: Some(4),
        };
        // Own units 1 and 2 (2 was launched by 1); enemy units 3 and 4 (4 is hidden).
        let units = [
            unit(1, player, 9),
            unit(2, player, 10),
            unit(3, enemy, 11),
            unit(4, enemy, 12),
        ];
        let view = |special: &crate::SpecialState| {
            let view = View::new(
                player,
                0,
                &players,
                state,
                &grid,
                &definitions,
                &plan,
                false,
                &units,
                &[],
                special,
            );
            (
                view.own_units
                    .iter()
                    .map(|unit| unit.id.0)
                    .collect::<Vec<_>>(),
                view.enemy_units
                    .iter()
                    .map(|unit| unit.id.0)
                    .collect::<Vec<_>>(),
                view.last_resort,
                // A cell that only unit 2 lights: (14, 10) is 4 cells from it and 5 from 1.
                view.stays_visible(CellPos::new(14, 10)),
            )
        };
        assert_eq!(
            view(&crate::SpecialState::default()),
            (vec![1, 2], vec![3, 4], false, true)
        );
        let mut special = crate::SpecialState::default();
        special.hidden.insert((player, SimId(4)));
        // Hidden from another player only: still seen by this one.
        special.hidden.insert((enemy, SimId(3)));
        special.spawned.insert(SimId(2), SimId(1));
        assert_eq!(view(&special), (vec![1], vec![3], false, false));
        // With only a launched unit armed, the player has no army of its own.
        special.spawned.insert(SimId(1), SimId(9));
        assert_eq!(view(&special), (vec![], vec![3], true, false));
    }
    #[test]
    fn ra6_shipped_utility_weapons_do_not_recruit_specialists_into_the_army() {
        let definitions = crate::load_ruleset(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/data"),
        )
        .unwrap();
        for id in ["defuse_kit", "makeup_kit", "assault_bolt", "psi_wave"] {
            assert!(
                !army_weapon(&definitions, definitions.weapon_kind(id).unwrap()),
                "{id}"
            );
        }
        assert!(army_weapon(
            &definitions,
            definitions.weapon_kind("fakec4").unwrap()
        ));
        for (id, expected) in [
            ("engineer", false),
            ("spy", false),
            ("tesla_trooper", true),
            ("yuri", true),
            ("chrono_legionnaire", true),
            ("crazy_ivan", true),
            ("tanya", true),
            ("chrono_commando", true),
            ("psi_commando", true),
            ("chrono_ivan", true),
            ("yuri_prime", true),
        ] {
            let terms = definitions
                .unit_combat(definitions.unit_kind(id).unwrap())
                .unwrap();
            assert_eq!(
                [terms.weapon, terms.secondary]
                    .into_iter()
                    .flatten()
                    .any(|kind| army_weapon(&definitions, kind)),
                expected,
                "{id}"
            );
        }
    }
    #[test]
    fn ra6_deployed_and_temporally_frozen_units_wait_for_release_before_army_orders() {
        let id = SimId(7);
        let mut special = crate::SpecialState::default();
        assert!(army_commandable(&special, id));
        special.deployed.insert(id, 0);
        assert!(!army_commandable(&special, id));
        special.deployed.clear();
        special.erasures.insert(
            id,
            crate::special::infantry::Erasure {
                remaining: 100,
                beams: BTreeMap::new(),
            },
        );
        assert!(!army_commandable(&special, id));
        special.erasures.clear();
        assert!(army_commandable(&special, id));
    }
}
