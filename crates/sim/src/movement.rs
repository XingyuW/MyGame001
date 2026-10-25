//! Movement (PLAN.md section 7, M2; docs/DECISIONS.md D19).
//!
//! Each tick, every unit in ascending id order advances by its speed in subcells along its
//! path. Before it leaves a cell centre it reserves the next cell in the grid (cell reservation
//! before entry), and it releases the cell it left when it reaches the next centre, so a cell is
//! never held by two units (invariant I6). Speed left over at a centre carries on into the next
//! step, so a unit covers exactly `speed` subcells per tick while it is not blocked.
//!
//! A unit whose next cell is held by another unit waits; after
//! [`REPATH_AFTER_BLOCKED_TICKS`] ticks of waiting it requests a path around waiting units. A
//! next cell that has become impassable (or a diagonal step that terrain now cuts) makes it
//! request a new path at once.
//!
//! Every order ends (docs/DECISIONS.md D21 and D22), and the rule that ends it measures
//! progress toward the destination, not movement, and never elapsed time whose length depends
//! on a unit's speed (a step takes from 2 ticks at speed 128 to 362 ticks at speed 1):
//!
//! * A step along a path that leads to the destination brings the unit closer along that path.
//!   An order ends with [`MoveFailure::Blocked`] when its unit has waited [`MAX_STALLED_TICKS`]
//!   ticks at a cell centre for cells held by units that are not moving on, without a step
//!   toward the destination in between. Ticks in a step, ticks spent waiting for the
//!   pathfinder, and ticks spent waiting behind units that are moving on (a unit in a step, or a
//!   queue whose head is in one) are not stalled ticks. A step aside does not reset the count,
//!   so units that keep stepping aside for each other do not keep their orders alive.
//! * Real progress is a step that starts with less remaining path cost to the destination than
//!   any earlier step of the order ([`MoveOrder::best`]). A detour around other units can make
//!   the remaining cost grow again, so a unit may take only [`MAX_PATHS_WITHOUT_PROGRESS`] new
//!   paths without real progress; then its order ends with [`MoveFailure::Blocked`]. Units that
//!   take turns giving way to each other, back and forth, therefore stop after a bounded number
//!   of turns. A path served while the unit waits behind units that are moving on, and that only
//!   confirms the way it already follows, is not a new path.
//!
//! Real progress lowers `best`, which cannot fall below zero; between two real progresses a
//! unit takes a bounded number of new paths and so a bounded number of steps, each of at most
//! 362 ticks; between two steps toward the destination it waits at most `MAX_STALLED_TICKS`
//! stalled ticks; a wait for the pathfinder is bounded by the request queue; and a wait behind
//! units that are moving on lasts only while some unit is in a step, which, with no new command,
//! happens in a bounded number of ticks because every unit's order takes a bounded number of
//! steps. So every order ends.
//!
//! Since RA3 (invariant I6', docs/DECISIONS.md R6):
//!
//! * **Infantry on foot** hold one of the three spots of a cell instead of the cell
//!   (`crate::map`). A soldier reserves a spot of the next cell before it leaves its own, keeps
//!   the spot it had if that one is free there, and walks from spot to spot in the ticks a step
//!   from centre to centre takes. It waits for a cell that a vehicle holds or whose three spots
//!   are held, and a vehicle waits for a cell in which any soldier holds a spot; the rules
//!   above are otherwise the same for both. A soldier that waits for a full cell waits behind
//!   units that are moving on if one of the three is moving on; a vehicle, if all are.
//! * **Crushing.** A unit whose move order may crush ([`MoveOrder::crush`]: the move of a `Move`
//!   or `AttackMove` command to a crusher) does not wait for a cell held only by crushable
//!   infantry of its enemies ([`Crushable`]): it drives in, and they die as it starts the step
//!   ([`SimEvent::UnitCrushed`], then [`SimEvent::UnitDestroyed`]), so that the cell never holds
//!   a vehicle and infantry together. Its own, an ally's, infantry that is not crushable, and
//!   a cell that also holds any of these, block it like any other unit.

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::query::With;
use bevy_ecs::system::{Commands, Query, Res, ResMut};

use crate::command::PlayerId;
use crate::definitions::{Definitions, UnitKind};
use crate::economy::Players;
use crate::event::{MoveFailure, SimEvent};
use crate::id::SimId;
use crate::map::{CellPos, Grid, SPOTS_PER_CELL};
use crate::pathfinding::{DIAGONAL_COST, ORTHOGONAL_COST};
use crate::schedule::SimTick;
use crate::unit::{
    DIAGONAL_STEP, Direction, Facing, Mobile, MoveOrder, ORTHOGONAL_STEP, Orders, Owner, PathState,
    Position, Step, Unit,
};

/// Ticks a unit waits for a cell held by another unit before it requests a path around waiting
/// units (and again after each further such number of ticks).
pub const REPATH_AFTER_BLOCKED_TICKS: u16 = 8;

/// Stalled ticks (see the module documentation: ticks spent waiting at a cell centre for cells
/// held by units that are not moving on) without a step toward the destination in between,
/// after which a move order ends with [`MoveFailure::Blocked`] (10 seconds). Units that wait for
/// each other with no way around (for example, head on in a one-cell corridor) therefore never
/// wait forever. A unit's own steps and waits behind units that are moving on do not count,
/// because their length depends on unit speed (docs/DECISIONS.md D22).
pub const MAX_STALLED_TICKS: u16 = 200;

/// New paths a unit may take since its last real progress (a step that left less path cost to
/// the destination than any earlier step of the order) before its order ends with
/// [`MoveFailure::Blocked`]. Units that keep giving way to each other without getting closer (for
/// example, two units that want to swap places in a dead end one cell wide) therefore never keep
/// their orders forever.
pub const MAX_PATHS_WITHOUT_PROGRESS: u16 = 64;

type MoveQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static SimId,
        &'static mut Mobile,
        &'static mut Position,
        &'static mut Facing,
        &'static mut Orders,
        &'static Owner,
        &'static Unit,
    ),
    With<Unit>,
>;

/// The infantry a crusher may drive over: every unit on foot of a crushable type, with its
/// entity and owner (RA3). Empty in a world without crushers, where nothing is ever crushed.
#[derive(Default)]
pub(crate) struct Crushable {
    units: BTreeMap<SimId, (Entity, PlayerId, UnitKind)>,
}

impl Crushable {
    /// Of `units` (entity, id, owner, type) those a crusher may drive over.
    pub(crate) fn of(
        definitions: &Definitions,
        units: impl Iterator<Item = (Entity, SimId, PlayerId, UnitKind)>,
    ) -> Self {
        if !definitions.has_crushers() {
            return Self::default();
        }
        Self {
            units: units
                .filter(|&(.., kind)| definitions.crushable(kind))
                .map(|(entity, id, owner, kind)| (id, (entity, owner, kind)))
                .collect(),
        }
    }

    /// Whether a crusher of `owner` may drive over `victim`: crushable infantry of an enemy
    /// ([`Players::hostile`]), never its own or an ally's.
    pub(crate) fn by(&self, players: &Players, owner: PlayerId, victim: SimId) -> bool {
        self.units
            .get(&victim)
            .is_some_and(|&(_, other, _)| players.hostile(owner, other))
    }
}

/// What a unit at a cell centre does next.
enum AtCentre {
    /// Stop moving this tick.
    Wait,
    /// Stop moving this tick because units that are not moving on hold the way: a stalled
    /// tick.
    Blocked,
    /// Start a step in this direction; `crush`: over the infantry that hold the cell.
    Step {
        direction: Direction,
        next: CellPos,
        crush: bool,
    },
}

/// What a unit is doing, as far as a unit that waits for a cell it holds needs to know.
#[derive(Clone, Copy)]
enum Activity {
    /// In a step, which it always completes within at most 362 ticks: it is moving on.
    Stepping,
    /// At a cell centre, waiting to enter `next`: the next cell of the path it follows, or of
    /// the path it had while it waits for a new one.
    Waiting { next: CellPos },
    /// Idle, ending its order, or without a path: it stays where it is.
    Standing,
}

impl Activity {
    fn of(mobile: &Mobile, orders: &Orders) -> Self {
        if mobile.step.is_some() {
            return Activity::Stepping;
        }
        orders
            .current
            .as_ref()
            .and_then(|order| order.path.last())
            .map_or(Activity::Standing, |&next| Activity::Waiting { next })
    }
}

/// Whether `blocker`, which holds a cell that a unit waits for, is moving on: it is in a step,
/// or it waits at a cell centre for a cell that has room for it or whose holders make room for
/// it in turn ([`make_room`]: a queue whose head is in a step). A chain that comes back to a
/// unit already `visited` (which starts as the unit that asks) is a wait-for cycle, and a chain
/// that ends at a unit that stands (or wants a cell that has become impassable) is blocked:
/// neither moves on.
fn moving_on(
    grid: &Grid,
    activity: &BTreeMap<SimId, Activity>,
    visited: &mut BTreeSet<SimId>,
    blocker: SimId,
) -> bool {
    if !visited.insert(blocker) {
        return false;
    }
    match activity.get(&blocker) {
        Some(Activity::Stepping) => true,
        Some(&Activity::Waiting { next }) => {
            if !grid.is_passable(next) {
                return false;
            }
            let on_foot = grid.is_on_foot(blocker);
            let blockers = grid.blockers(next, blocker, on_foot);
            make_room(grid, activity, visited, on_foot, blockers)
        }
        Some(Activity::Standing) | None => false,
    }
}

/// Whether the `blockers` of a cell (the units that keep a unit out of it, [`Grid::blockers`])
/// make room for that unit by moving on: all of them for a unit that needs the whole cell; one
/// of them for a unit on foot (`on_foot`), which needs one spot. No blockers: there is room.
fn make_room(
    grid: &Grid,
    activity: &BTreeMap<SimId, Activity>,
    visited: &mut BTreeSet<SimId>,
    on_foot: bool,
    blockers: [Option<SimId>; SPOTS_PER_CELL],
) -> bool {
    let mut blockers = blockers.into_iter().flatten().peekable();
    if blockers.peek().is_none() {
        return true;
    }
    if on_foot {
        blockers.any(|blocker| moving_on(grid, activity, visited, blocker))
    } else {
        blockers.all(|blocker| moving_on(grid, activity, visited, blocker))
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn move_units(
    mut commands: Commands,
    mut grid: ResMut<Grid>,
    tick: Res<SimTick>,
    mut events: ResMut<Messages<SimEvent>>,
    players: Res<Players>,
    definitions: Res<Definitions>,
    mut kills: ResMut<crate::veterancy::Kills>,
    mut units: MoveQuery,
    healths: Query<&crate::combat::Health>,
) {
    let alive = |entity| {
        !healths
            .get(entity)
            .is_ok_and(|health| health.hit_points <= 0)
    };
    let mut order: Vec<(SimId, Entity)> = units
        .iter()
        .filter(|(entity, ..)| alive(*entity))
        .map(|(entity, id, ..)| (*id, entity))
        .collect();
    order.sort_unstable();
    // What every unit is doing, kept up to date as the units move in id order.
    let mut activity: BTreeMap<SimId, Activity> = units
        .iter()
        .filter(|(entity, ..)| alive(*entity))
        .map(|(_, id, mobile, _, _, orders, ..)| (*id, Activity::of(mobile, orders)))
        .collect();
    let crushable = Crushable::of(
        &definitions,
        units
            .iter()
            .filter(|(entity, ..)| alive(*entity))
            .map(|(entity, id, .., owner, unit)| (entity, *id, owner.0, unit.kind)),
    );
    // The units crushed in this tick: they are gone, though their entities remain until the
    // commands of this system are applied.
    let mut crushed: BTreeSet<SimId> = BTreeSet::new();
    for (id, entity) in order {
        if crushed.contains(&id) {
            continue;
        }
        grid.navigation = grid.domain_of(id);
        let Ok((_, _, mut mobile, mut position, mut facing, mut orders, owner, _)) =
            units.get_mut(entity)
        else {
            continue;
        };
        // The owner's unit speed option scales the distance covered per tick; a unit may then
        // finish more than one step in a tick, which the loop below already allows.
        let mut budget =
            u32::from(mobile.speed).saturating_mul(players.unit_speed_percent(owner.0)) / 100;
        let mut stepped_toward = false;
        let mut stalled = false;
        loop {
            if let Some(mut step) = mobile.step {
                let remaining = step.length.saturating_sub(step.progress);
                let advance = u16::try_from(budget).unwrap_or(u16::MAX).min(remaining);
                step.progress = step.progress.saturating_add(advance);
                budget = budget.saturating_sub(u32::from(advance));
                if step.progress < step.length {
                    mobile.step = Some(step);
                    break;
                }
                let left = mobile.cell;
                grid.release(left, id);
                mobile.cell = step.to;
                mobile.step = None;
                continue;
            }
            match at_centre(
                id,
                &mobile,
                &mut orders,
                &grid,
                &activity,
                tick.0,
                budget,
                &mut events,
                |victim| crushable.by(&players, owner.0, victim),
            ) {
                AtCentre::Wait => break,
                AtCentre::Blocked => {
                    stalled = true;
                    break;
                }
                AtCentre::Step {
                    direction,
                    next,
                    crush,
                } => {
                    if crush {
                        // The infantry in the cell die as the crusher drives in: their spots
                        // are freed before it claims the cell, so that the cell never holds
                        // both (invariant I6').
                        for victim in grid.blockers(next, id, false).into_iter().flatten() {
                            let Some(&(victim_entity, victim_owner, victim_kind)) =
                                crushable.units.get(&victim)
                            else {
                                continue;
                            };
                            // The crusher is the killer (RA4: its experience), unless the
                            // victim's hit points were gone already: what destroyed it earlier
                            // in the tick has its kill, or it has no killer.
                            if !healths
                                .get(victim_entity)
                                .is_ok_and(|health| health.hit_points <= 0)
                            {
                                kills.record(
                                    id,
                                    victim_owner,
                                    crate::veterancy::worth(
                                        &definitions,
                                        crate::ProductionItem::Unit(victim_kind),
                                    ),
                                );
                            }
                            let at = grid.standing_point(next, victim);
                            grid.release_around(next, victim);
                            grid.unregister(victim);
                            crushed.insert(victim);
                            activity.remove(&victim);
                            commands.entity(victim_entity).despawn();
                            events.write(SimEvent::UnitCrushed {
                                player: victim_owner,
                                unit: victim,
                                crusher: id,
                                by: owner.0,
                                at,
                            });
                            events.write(SimEvent::UnitDestroyed {
                                player: victim_owner,
                                unit: victim,
                            });
                        }
                    }
                    // `at_centre` checked that the cell is passable and has room. A unit on
                    // foot keeps the spot it has, if that one is free in the next cell.
                    let spot = grid.spot(mobile.cell, id);
                    if !grid.claim_spot(next, id, spot) {
                        break;
                    }
                    mobile.step = Some(Step {
                        to: next,
                        progress: 0,
                        length: if direction.is_diagonal() {
                            DIAGONAL_STEP
                        } else {
                            ORTHOGONAL_STEP
                        },
                    });
                    facing.0 = direction;
                    if let Some(order) = orders.current.as_mut() {
                        order.path.pop();
                        order.blocked_ticks = 0;
                        stepped_toward |= start_step(order, direction);
                    }
                }
            }
        }
        if let Some(order) = orders.current.as_mut() {
            if stepped_toward {
                order.stalled_ticks = 0;
            } else if stalled {
                order.stalled_ticks = order.stalled_ticks.saturating_add(1);
            }
            let stuck = order.stalled_ticks >= MAX_STALLED_TICKS
                || order.paths_since_progress > MAX_PATHS_WITHOUT_PROGRESS;
            if stuck && mobile.step.is_none() {
                orders.current = None;
                events.write(SimEvent::MoveFailed {
                    unit: id,
                    reason: MoveFailure::Blocked,
                });
            }
        }
        activity.insert(id, Activity::of(&mobile, &orders));
        grid.navigation = crate::Domain::Land;
        let sub = mobile.position(&grid, id);
        if position.0 != sub {
            position.0 = sub;
        }
    }
}

/// Records a step that the unit has just started along its order's path, and returns whether
/// it is a step toward the destination: along a path that leads there (a step aside is not).
/// A step that leaves less path cost to the destination than any earlier step of the order is
/// real progress: it lowers `best` and clears `paths_since_progress`.
fn start_step(order: &mut MoveOrder, direction: Direction) -> bool {
    let cost = if direction.is_diagonal() {
        DIAGONAL_COST
    } else {
        ORTHOGONAL_COST
    };
    order.remaining = order
        .remaining
        .map(|remaining| remaining.saturating_sub(cost));
    let Some(remaining) = order.remaining else {
        return false;
    };
    if remaining < order.best {
        order.best = remaining;
        order.paths_since_progress = 0;
    }
    true
}

/// The decision of a unit standing at the centre of its cell (on foot: at its spot) with
/// `budget` subcells left. `crushes` says whether the unit, if its order may crush, may drive
/// over a unit.
#[allow(clippy::too_many_arguments)]
fn at_centre(
    id: SimId,
    mobile: &Mobile,
    orders: &mut Orders,
    grid: &Grid,
    activity: &BTreeMap<SimId, Activity>,
    tick: u64,
    budget: u32,
    events: &mut Messages<SimEvent>,
    crushes: impl Fn(SimId) -> bool,
) -> AtCentre {
    let Some(order) = orders.current.as_mut() else {
        return AtCentre::Wait;
    };
    if order.state != PathState::Following {
        return AtCentre::Wait;
    }
    let request = |order: &mut MoveOrder, avoid_units: bool, queued: bool| {
        order.state = PathState::Requested {
            since: tick,
            avoid_units,
            queued,
        };
        order.remaining = None;
        order.blocked_ticks = 0;
        order.repaths = order.repaths.saturating_add(1);
    };
    let Some(&next) = order.path.last() else {
        if mobile.cell == order.destination {
            let retargeted = order.retargets > 0;
            orders.current = None;
            events.write(SimEvent::MoveCompleted {
                unit: id,
                retargeted,
            });
        } else {
            request(order, false, false);
        }
        return AtCentre::Wait;
    };
    if budget == 0 {
        return AtCentre::Wait;
    }
    let cell = mobile.cell;
    let Some(direction) =
        Direction::from_delta(next.x.saturating_sub(cell.x), next.y.saturating_sub(cell.y))
    else {
        request(order, false, false);
        return AtCentre::Wait;
    };
    let (dx, dy) = direction.delta();
    let beside_x = CellPos::new(cell.x.saturating_add(dx), cell.y);
    let beside_y = CellPos::new(cell.x, cell.y.saturating_add(dy));
    // Terrain that changed since the path was found: the next cell, or for a diagonal step
    // both cells it passes between (PLAN.md section 5, equation 5).
    let cut = direction.is_diagonal() && !grid.is_passable(beside_x) && !grid.is_passable(beside_y);
    if !grid.is_passable(next) || cut {
        // The terrain changed under the path, so the cost of every way to the destination may
        // have grown: progress is measured afresh from the new path.
        order.best = u32::MAX;
        request(order, false, false);
        return AtCentre::Wait;
    }
    // Other units leave the unit no room in the next cell, or a unit is crossing this diagonal
    // step between the two cells beside it (it holds both).
    let on_foot = grid.is_on_foot(id);
    let mut blockers = grid.blockers(next, id, on_foot);
    let across = grid.holders(beside_y);
    let crossers = grid.holders(beside_x).map(|holder| {
        holder.filter(|holder| {
            direction.is_diagonal() && *holder != id && across.contains(&Some(*holder))
        })
    });
    // A unit whose order may crush drives into a cell held only by infantry it may crush.
    let crush = order.crush
        && blockers.iter().any(Option::is_some)
        && blockers.into_iter().flatten().all(&crushes);
    if crush {
        blockers = [None; SPOTS_PER_CELL];
    }
    if blockers.iter().chain(&crossers).any(Option::is_some) {
        // Each unit in the way is followed on its own, as far as the units it waits for.
        let moves_on =
            |blocker: SimId| moving_on(grid, activity, &mut BTreeSet::from([id]), blocker);
        let mut holders = blockers.into_iter().flatten().peekable();
        let room_soon = holders.peek().is_none()
            || if on_foot {
                holders.any(moves_on)
            } else {
                holders.all(moves_on)
            };
        let queued = room_soon && crossers.into_iter().flatten().all(moves_on);
        order.blocked_ticks = order.blocked_ticks.saturating_add(1);
        if order.blocked_ticks >= REPATH_AFTER_BLOCKED_TICKS {
            request(order, true, queued);
        }
        return if queued {
            AtCentre::Wait
        } else {
            AtCentre::Blocked
        };
    }
    AtCentre::Step {
        direction,
        next,
        crush,
    }
}
