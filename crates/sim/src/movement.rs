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
//! Every order ends (docs/DECISIONS.md D21), and the rule that ends it measures progress toward
//! the destination, not movement:
//!
//! * A step along a path that leads to the destination brings the unit closer along that path.
//!   An order whose unit takes no such step for [`MAX_STALLED_TICKS`] ticks, not counting ticks
//!   spent waiting for the pathfinder, ends with [`MoveFailure::Blocked`]. A step aside does not
//!   count, so units that keep stepping aside for each other do not keep their orders alive.
//! * Real progress is a step that starts with less remaining path cost to the destination than
//!   any earlier step of the order ([`MoveOrder::best`]). A detour around other units can make
//!   the remaining cost grow again, so a unit may take only [`MAX_PATHS_WITHOUT_PROGRESS`] new
//!   paths without real progress; then its order ends with [`MoveFailure::Blocked`]. Units that
//!   take turns giving way to each other, back and forth, therefore stop after a bounded number
//!   of turns.
//!
//! Real progress lowers `best`, which cannot fall below zero; between two real progresses a
//! unit takes a bounded number of paths, and on each path it either steps toward the destination
//! or waits a bounded time. So every order ends within a bounded number of ticks.

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::query::With;
use bevy_ecs::system::{Query, Res, ResMut};

use crate::event::{MoveFailure, SimEvent};
use crate::id::SimId;
use crate::map::{CellPos, Grid};
use crate::pathfinding::{DIAGONAL_COST, ORTHOGONAL_COST};
use crate::schedule::SimTick;
use crate::unit::{
    DIAGONAL_STEP, Direction, Facing, Mobile, MoveOrder, ORTHOGONAL_STEP, Orders, PathState,
    Position, Step, Unit,
};

/// Ticks a unit waits for a cell held by another unit before it requests a path around waiting
/// units.
pub const REPATH_AFTER_BLOCKED_TICKS: u16 = 8;

/// Ticks without a step toward the destination (see the module documentation), not counting
/// ticks spent waiting for the pathfinder, after which a move order ends with
/// [`MoveFailure::Blocked`] (10 seconds). Units that wait for each other with no way around (for
/// example, head on in a one-cell corridor) therefore never wait forever.
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
    ),
    With<Unit>,
>;

/// What a unit at a cell centre does next.
enum AtCentre {
    /// Stop moving this tick.
    Wait,
    /// Start a step in this direction.
    Step(Direction, CellPos),
}

pub(crate) fn move_units(
    mut grid: ResMut<Grid>,
    tick: Res<SimTick>,
    mut events: ResMut<Messages<SimEvent>>,
    mut units: MoveQuery,
) {
    let mut order: Vec<(SimId, Entity)> =
        units.iter().map(|(entity, id, ..)| (*id, entity)).collect();
    order.sort_unstable();
    for (id, entity) in order {
        let Ok((_, _, mut mobile, mut position, mut facing, mut orders)) = units.get_mut(entity)
        else {
            continue;
        };
        let mut budget = u32::from(mobile.speed);
        let mut stepped_toward = false;
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
            match at_centre(id, &mobile, &mut orders, &grid, tick.0, budget, &mut events) {
                AtCentre::Wait => break,
                AtCentre::Step(direction, next) => {
                    // `at_centre` checked that the cell is passable and free.
                    if !grid.claim(next, id) {
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
            } else if order.state == PathState::Following {
                // Ticks spent waiting for the pathfinder do not count: requests are served
                // oldest first, so each wait is bounded by the number of units.
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
        let sub = mobile.sub_position();
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

/// The decision of a unit standing at the centre of its cell with `budget` subcells left.
fn at_centre(
    id: SimId,
    mobile: &Mobile,
    orders: &mut Orders,
    grid: &Grid,
    tick: u64,
    budget: u32,
    events: &mut Messages<SimEvent>,
) -> AtCentre {
    let Some(order) = orders.current.as_mut() else {
        return AtCentre::Wait;
    };
    if order.state != PathState::Following {
        return AtCentre::Wait;
    }
    let request = |order: &mut MoveOrder, avoid_units: bool| {
        order.state = PathState::Requested {
            since: tick,
            avoid_units,
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
            request(order, false);
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
        request(order, false);
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
        request(order, false);
        return AtCentre::Wait;
    }
    // Another unit holds the next cell, or is crossing this diagonal step between the two
    // cells beside it.
    let held = grid.occupant(next).is_some_and(|holder| holder != id);
    let crossing = direction.is_diagonal()
        && grid
            .occupant(beside_x)
            .is_some_and(|holder| holder != id && grid.occupant(beside_y) == Some(holder));
    if held || crossing {
        order.blocked_ticks = order.blocked_ticks.saturating_add(1);
        if order.blocked_ticks >= REPATH_AFTER_BLOCKED_TICKS {
            request(order, true);
        }
        return AtCentre::Wait;
    }
    AtCentre::Step(direction, next)
}
