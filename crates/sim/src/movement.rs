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

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::query::With;
use bevy_ecs::system::{Query, Res, ResMut};

use crate::event::{MoveFailure, SimEvent};
use crate::id::SimId;
use crate::map::{CellPos, Grid};
use crate::schedule::SimTick;
use crate::unit::{
    DIAGONAL_STEP, Direction, Facing, Mobile, ORTHOGONAL_STEP, Orders, PathState, Position, Step,
    Unit,
};

/// Ticks a unit waits for a cell held by another unit before it requests a path around waiting
/// units.
pub const REPATH_AFTER_BLOCKED_TICKS: u16 = 8;

/// Consecutive ticks without any advance after which a move order ends with
/// [`MoveFailure::Blocked`] (10 seconds). Units waiting for each other with no way around (for
/// example, head on in a one-cell corridor) therefore never wait forever: every order ends.
pub const MAX_STALLED_TICKS: u16 = 200;

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
        let mut advanced = false;
        loop {
            if let Some(mut step) = mobile.step {
                let remaining = step.length.saturating_sub(step.progress);
                let advance = u16::try_from(budget).unwrap_or(u16::MAX).min(remaining);
                advanced |= advance > 0;
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
                    }
                }
            }
        }
        if let Some(order) = orders.current.as_mut() {
            if advanced {
                order.stalled_ticks = 0;
            } else {
                order.stalled_ticks = order.stalled_ticks.saturating_add(1);
            }
            if order.stalled_ticks >= MAX_STALLED_TICKS && mobile.step.is_none() {
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
    let request = |order: &mut crate::unit::MoveOrder, avoid_units: bool| {
        order.state = PathState::Requested {
            since: tick,
            avoid_units,
        };
        order.blocked_ticks = 0;
        order.repaths = order.repaths.saturating_add(1);
    };
    let Some(&next) = order.path.last() else {
        if mobile.cell == order.destination {
            orders.current = None;
            events.write(SimEvent::MoveCompleted { unit: id });
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
