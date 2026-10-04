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
//! 1. places a ready building on a free spot near its base ([`find_placement`]);
//! 2. queues the next building of the build order that it does not own as often as the build
//!    order lists it (so lost buildings are rebuilt), or a power building first if the power would
//!    not suffice; but when it has no armed unit, no working harvester, and too few credits for the
//!    army unit it can produce, it sells a building its army does not need instead, and queues no
//!    building until it has an armed unit again ([`broke`]);
//! 3. queues harvesters up to `harvesters_per_refinery` per refinery, and armed units of the army
//!    composition up to `max_army`, at most [`AI_UNIT_QUEUE`] items per producing building type;
//! 4. sets the rally point of its new unit producers to its gathering point, and sends idle
//!    harvesters back to work, at most once every [`HARVEST_RETRY_TICKS`];
//! 5. defends: visible enemies within `defence_radius` of its buildings are attacked by its army
//!    at home; otherwise idle units at home gather at the gathering point;
//! 6. attacks: from tick `first_wave`, once `wave_size` armed units are at home, or any after
//!    `wave_patience` more ticks, they join the attack wave, which attack-moves to the nearest
//!    building it remembers, else to an enemy start position it has never seen, else sweeps the
//!    map's waypoints, the one seen longest ago first; a wave reduced below half the wave size
//!    falls back to the gathering point and rejoins the units at home.

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::query::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Query, Res, ResMut};

use crate::ai_defs::{AiDefinitions, AiPlan, Difficulty};
use crate::building::{Building, Footprint, Producer};
use crate::combat::{Sight, Weapon};
use crate::command::{Command, CommandQueue, IssuedCommand, PlayerId};
use crate::definitions::{Definitions, UnitKind};
use crate::economy::{MAX_QUEUE_LENGTH, PlayerState, Players};
use crate::economy_commands::check_placement;
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

/// A building another player owns that a computer player has seen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KnownBuilding {
    pub owner: PlayerId,
    pub footprint: Footprint,
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
    wave: BTreeSet<SimId>,
    target: Option<WaveTarget>,
    failures: BTreeMap<SimId, u8>,
    next_army: usize,
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
}

impl AiController {
    /// A controller following `plan` (the AI file's plan for its faction, `AiDefinitions::plan_for`)
    /// with `difficulty` (one of the AI file's difficulties) for a player starting at `start`,
    /// whose enemies start at `enemy_starts`; match setup has checked that every start position is
    /// a cell of the map with passable terrain. The gathering point and the waypoints come from the
    /// static terrain of `grid` (docs/DECISIONS.md D43), so they lie on the map. `None` if no
    /// gathering point exists ([`gathering_point`]), which match setup reports as an error instead
    /// of falling back to a cell its commands could not use (M6 review 1 P1-1).
    pub(crate) fn new(
        grid: &Grid,
        definitions: &Definitions,
        plan: AiPlan,
        difficulty: Difficulty,
        start: CellPos,
        enemy_starts: Vec<CellPos>,
    ) -> Option<Self> {
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
        let gather = gathering_point(grid, start, toward)?;
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
            wave: BTreeSet::new(),
            target: None,
            failures: BTreeMap::new(),
            next_army: 0,
            patience_until: u64::from(difficulty.first_wave)
                .saturating_add(u64::from(difficulty.wave_patience)),
            next_harvest: 0,
            fall_back_exempt: false,
            liquidating: false,
            issued: 0,
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
            write_cell(hasher, known.footprint.origin);
            hasher.write(&[known.footprint.width, known.footprint.height]);
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
        hasher.write_u64(self.patience_until);
        hasher.write_u64(self.next_harvest);
        hasher.write(&[u8::from(self.fall_back_exempt), u8::from(self.liquidating)]);
        hasher.write_u64(self.issued);
    }
}

/// The gathering point of a player starting at `start` (a cell of the map with passable terrain):
/// of the cells reachable from `start` over passable terrain (four-neighbour connectivity, which
/// for the eight-neighbour steps of equation 5, with no diagonal step between two blocked
/// orthogonal neighbours, connects the same cells), those that no building covers, the one nearest
/// to `toward` (octile distance, then row, then column). `None` if there is none, for example a
/// start walled in by terrain under its own buildings. Its units can walk there unless buildings
/// placed later block the way.
fn gathering_point(grid: &Grid, start: CellPos, toward: CellPos) -> Option<CellPos> {
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
        if grid.building(cell).is_none() {
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

// --- What the controllers see ---------------------------------------------------------------

/// A unit, as read at the start of the `Ai` stage.
struct UnitSnap {
    id: SimId,
    owner: PlayerId,
    mobile: Mobile,
    has_order: bool,
    harvest: Option<HarvestState>,
    armed: bool,
    attacking: bool,
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
    tick: u64,
    state: &'a PlayerState,
    vision: &'a Vision,
    grid: &'a Grid,
    definitions: &'a Definitions,
    /// The computer player's plan (named `ai` as the AI file it is taken from).
    ai: &'a AiPlan,
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
        state: &'a PlayerState,
        grid: &'a Grid,
        definitions: &'a Definitions,
        ai: &'a AiPlan,
        units: &'a [UnitSnap],
        buildings: &'a [BuildingSnap],
    ) -> Self {
        let vision = state.vision();
        Self {
            tick,
            state,
            vision,
            grid,
            definitions,
            ai,
            own_units: units.iter().filter(|unit| unit.owner == player).collect(),
            own_buildings: buildings
                .iter()
                .filter(|building| building.owner == player)
                .collect(),
            enemy_units: units
                .iter()
                .filter(|unit| unit.owner != player && vision.sees_unit(&unit.mobile))
                .collect(),
            enemy_buildings: buildings
                .iter()
                .filter(|building| {
                    building.owner != player && vision.sees_footprint(building.footprint)
                })
                .collect(),
        }
    }

    fn owns(&self, kind: BuildingKind) -> bool {
        self.own_buildings
            .iter()
            .any(|building| building.kind == kind)
    }

    fn owned(&self, kind: BuildingKind) -> usize {
        self.own_buildings
            .iter()
            .filter(|building| building.kind == kind)
            .count()
    }

    /// Whether `QueueProduction` of `item` would pass its validation now: the player owns every
    /// prerequisite and a producer, and the queue is not full.
    fn queueable(&self, item: ProductionItem) -> bool {
        self.definitions.production(item).is_some_and(|terms| {
            terms.prerequisites.iter().all(|&kind| self.owns(kind))
                && self.owns(terms.producer)
                && self.state.queue(terms.producer).len() < MAX_QUEUE_LENGTH
        })
    }

    fn queue_len(&self, item: ProductionItem) -> usize {
        self.definitions
            .production(item)
            .map_or(usize::MAX, |terms| self.state.queue(terms.producer).len())
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
    /// steps into, `crate::movement`) (equation 6).
    fn stays_visible(&self, cell: CellPos) -> bool {
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
            unit.sight.is_some_and(|sight| {
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
        &'static Owner,
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
        &'static Owner,
        &'static Building,
        Option<&'static Producer>,
        Option<&'static Sight>,
    ),
>;

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
) {
    if ai.controllers.is_empty() || state.outcome().is_some() {
        return;
    }
    let tick = tick.0;
    let thinking: Vec<PlayerId> = ai
        .controllers
        .iter()
        .filter(|(player, controller)| {
            let interval = u64::from(controller.difficulty.think_interval.max(1));
            tick % interval == u64::from(player.0) % interval && state.defeated(**player).is_none()
        })
        .map(|(player, _)| *player)
        .collect();
    if thinking.is_empty() {
        return;
    }
    let mut unit_snaps: Vec<UnitSnap> = units
        .iter()
        .map(
            |(id, owner, mobile, orders, harvester, weapon, sight)| UnitSnap {
                id: *id,
                owner: owner.0,
                mobile: *mobile,
                has_order: orders.current.is_some(),
                harvest: harvester.map(|harvester| harvester.state),
                armed: weapon.is_some(),
                attacking: weapon.is_some_and(|weapon| weapon.order.is_some()),
                sight: sight.map(|sight| sight.0),
            },
        )
        .collect();
    unit_snaps.sort_unstable_by_key(|unit| unit.id);
    let mut building_snaps: Vec<BuildingSnap> = buildings
        .iter()
        .map(|(id, owner, building, producer, sight)| BuildingSnap {
            id: *id,
            owner: owner.0,
            kind: building.kind,
            footprint: building.footprint,
            producer: producer.map(|producer| producer.rally),
            sight: sight.map(|sight| sight.0),
            pending: building.pending_units.clone(),
        })
        .collect();
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
            player_state,
            &grid,
            &definitions,
            &plan,
            &unit_snaps,
            &building_snaps,
        );
        let commands = think(&view, controller);
        controller.issued = controller.issued.saturating_add(commands.len() as u64);
        for command in commands {
            queue.push(IssuedCommand { player, command });
        }
    }
}

/// One turn of a computer player: updates its memory and returns its commands.
fn think(view: &View<'_>, controller: &mut AiController) -> Vec<Command> {
    remember(view, controller);
    let mut commands = Vec::new();
    if let Some(command) = place_ready_building(view, controller) {
        commands.push(command);
    }
    // While it sells buildings to fund an army unit, and until it has one, it does not queue
    // buildings, which would take the credits first.
    if view.own_units.iter().any(|unit| unit.armed) {
        controller.liquidating = false;
    }
    if broke(view) {
        if let Some(command) = sell_for_army(view) {
            controller.liquidating = true;
            commands.push(command);
        }
    } else if !controller.liquidating
        && let Some(command) = queue_building(view)
    {
        commands.push(command);
    }
    commands.extend(queue_units(view, controller));
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
        controller.failures.remove(&id);
    }
    let stamp = view.tick.saturating_add(1);
    for waypoint in &mut controller.waypoints {
        if view.vision.is_visible(waypoint.cell) {
            waypoint.seen = stamp;
        }
    }
}

// --- Construction ---------------------------------------------------------------------------

/// `PlaceBuilding` for a building that is ready at the head of one of the player's queues, if a
/// place is found.
fn place_ready_building(view: &View<'_>, controller: &AiController) -> Option<Command> {
    let ready = view.state.queues().find_map(|(_, items)| {
        let head = items.first()?;
        let ProductionItem::Building(kind) = head.item else {
            return None;
        };
        let terms = view.definitions.production(head.item)?;
        (head.progress >= terms.build_time).then_some(kind)
    })?;
    let origin = find_placement(view, controller, ready)?;
    Some(Command::PlaceBuilding {
        building: ready,
        origin,
    })
}

/// A footprint origin for a building of type `kind` near the player's base that passes the
/// command's own validation now ([`check_placement`]) and stays valid until the command is
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
    let def = view.definitions.building(kind)?;
    let (width, height) = (i32::from(def.width.max(1)), i32::from(def.height.max(1)));
    let reach = i32::try_from(view.definitions.placement_distance()).unwrap_or(0);
    let mut origins: BTreeSet<CellPos> = BTreeSet::new();
    for building in &view.own_buildings {
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
    let mut candidates: Vec<(u32, u32, CellPos)> = origins
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
                to_ore,
                octile_distance(footprint.origin, controller.start),
                footprint.origin,
            )
        })
        .collect();
    candidates
        .sort_unstable_by_key(|&(to_ore, distance, origin)| (to_ore, distance, origin.y, origin.x));
    let owned: Vec<Footprint> = view
        .own_buildings
        .iter()
        .map(|building| building.footprint)
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
                && check_placement(
                    view.grid,
                    view.definitions,
                    view.state,
                    owned.iter().copied(),
                    kind,
                    origin,
                )
                .is_ok()
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

/// Whether the player can neither fight nor pay for a fighter, though it could produce one: it has
/// no armed unit, none of its harvesters is working (all have stopped, or it has none), it owns the
/// producer and prerequisites of some army unit type, and its credits are below what the cheapest
/// army unit in its queues still costs (or, with none queued, the cost of the cheapest one it can
/// produce). Without income its credits will not grow, so a match between such players would never
/// end (docs/DECISIONS.md D43).
fn broke(view: &View<'_>) -> bool {
    if view.own_units.iter().any(|unit| unit.armed)
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
        .filter_map(|&kind| view.definitions.production(ProductionItem::Unit(kind)))
        .filter(|terms| {
            view.owns(terms.producer) && terms.prerequisites.iter().all(|&kind| view.owns(kind))
        })
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
    let mut needed: BTreeSet<BuildingKind> = BTreeSet::new();
    for &kind in &view.ai.army {
        if let Some(terms) = view.definitions.production(ProductionItem::Unit(kind)) {
            needed.insert(terms.producer);
            needed.extend(terms.prerequisites.iter().copied());
        }
    }
    let builds = |kind: BuildingKind| {
        view.definitions
            .buildings()
            .iter()
            .any(|def| def.production.producer == kind)
    };
    view.own_buildings
        .iter()
        .filter(|building| !needed.contains(&building.kind))
        .max_by_key(|building| {
            let value = view
                .definitions
                .building(building.kind)
                .map_or(0, |def| def.sell_value);
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

/// The next building of the build order that the player does not own as often as the build order
/// lists it up to that entry, queued if its producer's queue is empty; a power building instead if
/// that building would leave the player without enough power, or if the build order is complete
/// and the player has low power.
fn queue_building(view: &View<'_>) -> Option<Command> {
    let order = &view.ai.build_order;
    let needed = order.iter().enumerate().find_map(|(index, &kind)| {
        let listed = order[..=index]
            .iter()
            .filter(|&&other| other == kind)
            .count();
        (view.owned(kind) < listed).then_some(kind)
    });
    let spare = view
        .state
        .power_produced()
        .saturating_sub(view.state.power_consumed());
    let kind = match needed {
        Some(kind) => {
            let consumed = view
                .definitions
                .building(kind)
                .map_or(0, |def| def.power_consumed);
            if kind != view.ai.power && spare < consumed {
                view.ai.power
            } else {
                kind
            }
        }
        None if view.state.low_power() => view.ai.power,
        None => return None,
    };
    let item = ProductionItem::Building(kind);
    (view.queue_len(item) == 0 && view.queueable(item)).then_some(Command::QueueProduction { item })
}

/// Harvesters up to the difficulty's number per refinery, then armed units of the army
/// composition up to its army size, at most [`AI_UNIT_QUEUE`] items per producing building type.
fn queue_units(view: &View<'_>, controller: &mut AiController) -> Vec<Command> {
    let mut commands = Vec::new();
    let mut added: BTreeMap<BuildingKind, usize> = BTreeMap::new();
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
    if harvesters < wanted && has_room(view, harvester, &mut added) {
        commands.push(Command::QueueProduction { item: harvester });
    }

    let army = &view.ai.army;
    let mut total = view.own_units.iter().filter(|unit| unit.armed).count()
        + army
            .iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|&kind| view.queued(ProductionItem::Unit(kind)))
            .sum::<usize>();
    let max = controller.difficulty.max_army as usize;
    for step in 0..army.len() {
        if total >= max {
            break;
        }
        let index = (controller.next_army + step) % army.len();
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
fn has_room(
    view: &View<'_>,
    item: ProductionItem,
    added: &mut BTreeMap<BuildingKind, usize>,
) -> bool {
    let Some(producer) = view
        .definitions
        .production(item)
        .map(|terms| terms.producer)
    else {
        return false;
    };
    let extra = added.get(&producer).copied().unwrap_or(0);
    if view.queue_len(item).saturating_add(extra) >= AI_UNIT_QUEUE || !view.queueable(item) {
        return false;
    }
    *added.entry(producer).or_insert(0) += 1;
    true
}

/// `SetRallyPoint` to the gathering point for every producer of units without a rally point.
fn set_rally_points(view: &View<'_>, controller: &AiController) -> Vec<Command> {
    view.own_buildings
        .iter()
        .filter(|building| building.producer == Some(None))
        .map(|building| Command::SetRallyPoint {
            building: building.id,
            cell: controller.gather,
        })
        .collect()
}

/// `Harvest` for the idle harvesters, at the refinery nearest to the start position, at most once
/// every [`HARVEST_RETRY_TICKS`].
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
        .filter(|unit| unit.armed)
        .map(|unit| (unit.id, *unit))
        .collect();
    controller.wave.retain(|id| armed.contains_key(id));
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
        .map(|(id, footprint)| (id, nearest_cell(footprint, controller.start)))
        .min_by_key(|&(id, cell)| (chebyshev(cell, controller.start), id));
    match threat {
        Some((_, cell)) => {
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

    // A wave reduced to less than half its size falls back to the gathering point and rejoins the
    // units at home, so that the next wave attacks with them at full size instead of feeding
    // units in one by one.
    if !controller.wave.is_empty()
        && controller.wave.len().saturating_mul(2) < controller.difficulty.wave_size as usize
        && !controller.fall_back_exempt
    {
        let back: Vec<SimId> = controller.wave.iter().copied().collect();
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
    let ready = home.len() >= controller.difficulty.wave_size as usize
        || (!home.is_empty() && view.tick >= controller.patience_until);
    if threat.is_none() && view.tick >= u64::from(controller.difficulty.first_wave) && ready {
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
    let at_rest = members.iter().all(|unit| unit.idle()) && joined.is_empty();
    let valid = match controller.target {
        Some(WaveTarget::Building(id)) => controller.known.contains_key(&id),
        Some(WaveTarget::Waypoint(_)) => true,
        None => false,
    };
    let centroid = centroid(&members, controller.start);
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
        controller.target = choose_target(controller, centroid);
        if let Some(cell) = target_cell(controller, centroid) {
            commands.push(Command::AttackMove {
                units: members.iter().map(|unit| unit.id).collect(),
                target: cell,
            });
        }
    } else if !joined.is_empty()
        && let Some(cell) = target_cell(controller, centroid)
    {
        commands.push(Command::AttackMove {
            units: joined,
            target: cell,
        });
    }
    commands
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
/// nearest enemy start position never seen; else the waypoint seen longest ago, nearest first.
fn choose_target(controller: &AiController, from: CellPos) -> Option<WaveTarget> {
    let building = controller
        .known
        .iter()
        .filter(|(id, _)| controller.failures.get(id).copied().unwrap_or(0) < MAX_TARGET_FAILURES)
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
    controller
        .waypoints
        .iter()
        .enumerate()
        .min_by_key(|(_, waypoint)| {
            (
                waypoint.seen,
                octile_distance(from, waypoint.cell),
                waypoint.cell.y,
                waypoint.cell.x,
            )
        })
        .map(|(index, _)| WaveTarget::Waypoint(index))
}

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
