//! Pathfinding (PLAN.md section 5, equation 5; docs/DECISIONS.md D19).
//!
//! A* on the eight-neighbour grid with step cost 10 orthogonal and 14 diagonal, no diagonal step
//! between two blocked orthogonal neighbours, and the octile heuristic
//! `h = 10 × (dx + dy) − 6 × min(dx, dy)`. The heuristic is consistent for these costs, so the
//! first time A* takes the goal from the open list its cost is optimal.
//!
//! Requests are served by [`find_paths`] in the `Pathfinding` stage, at most
//! [`PATH_REQUESTS_PER_TICK`] per tick, oldest first.
//!
//! Since RA3 (invariant I6', docs/DECISIONS.md R6) a cell held by units is closed to a unit if
//! those that count as obstacles leave it no room there ([`Grid::blocked`]): any of them for a
//! vehicle, a vehicle or all three spots for a unit on foot. Infantry that a unit whose order
//! may crush would drive over is no obstacle to it.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::query::With;
use bevy_ecs::system::{Query, Res, ResMut};

use crate::definitions::Definitions;
use crate::economy::Players;
use crate::event::{MoveFailure, SimEvent};
use crate::id::SimId;
use crate::map::{CellPos, Grid};
use crate::movement::Crushable;
use crate::unit::{Direction, Mobile, Orders, Owner, PathState, Unit};

/// Cost of an orthogonal step.
pub const ORTHOGONAL_COST: u32 = 10;

/// Cost of a diagonal step.
pub const DIAGONAL_COST: u32 = 14;

/// Path requests served per tick; further requests wait for later ticks, oldest first.
pub const PATH_REQUESTS_PER_TICK: usize = 16;

/// The octile distance between two cells: the cost of the cheapest path on an empty grid.
pub fn octile_distance(a: CellPos, b: CellPos) -> u32 {
    let dx = a.x.abs_diff(b.x);
    let dy = a.y.abs_diff(b.y);
    // Saturating: exact for any two cells of a map (at most 256 per side), and no overflow for
    // arbitrary cells (invariant I12).
    ORTHOGONAL_COST
        .saturating_mul(dx.saturating_add(dy))
        .saturating_sub(6_u32.saturating_mul(dx.min(dy)))
}

/// A path: the cells to enter in order (the start excluded, the goal last) and its cost.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Path {
    pub cells: Vec<CellPos>,
    pub cost: u32,
}

fn step(cell: CellPos, direction: Direction) -> CellPos {
    let (dx, dy) = direction.delta();
    CellPos::new(cell.x.saturating_add(dx), cell.y.saturating_add(dy))
}

/// The neighbours of `cell` that a unit may step to, with the step cost: inside the map,
/// `enterable`, and for a diagonal step, at least one of the two orthogonal neighbours it passes
/// between is `enterable`.
fn neighbours(
    grid: &Grid,
    cell: CellPos,
    enterable: &impl Fn(CellPos) -> bool,
) -> impl Iterator<Item = (CellPos, u32)> {
    Direction::ALL.into_iter().filter_map(move |direction| {
        let next = step(cell, direction);
        if !grid.contains(next) || !enterable(next) {
            return None;
        }
        if direction.is_diagonal() {
            let (dx, dy) = direction.delta();
            let beside_x = CellPos::new(cell.x.saturating_add(dx), cell.y);
            let beside_y = CellPos::new(cell.x, cell.y.saturating_add(dy));
            if !enterable(beside_x) && !enterable(beside_y) {
                return None;
            }
            Some((next, DIAGONAL_COST))
        } else {
            Some((next, ORTHOGONAL_COST))
        }
    })
}

fn cell_index(grid: &Grid, cell: CellPos) -> usize {
    // Only called for cells inside the map, whose coordinates are below 256.
    (cell.y as usize) * (grid.width() as usize) + (cell.x as usize)
}

fn cell_at(grid: &Grid, index: usize) -> CellPos {
    let width = (grid.width() as usize).max(1);
    // Indices are below width × height ≤ 65,536, so both coordinates fit in i32.
    CellPos::new((index % width) as i32, (index / width) as i32)
}

/// The cheapest path from `start` to `goal` over cells that are passable and not `blocked`, or
/// `None` if there is none. The start cell itself may be impassable (a unit may stand on a cell
/// that has just become blocked). Ties between equally cheap paths are broken by the open
/// list's order (f, then h, then cell index), so the result is deterministic.
pub fn find_path(
    grid: &Grid,
    start: CellPos,
    goal: CellPos,
    blocked: impl Fn(CellPos) -> bool,
) -> Option<Path> {
    if !grid.contains(start) || !grid.contains(goal) {
        return None;
    }
    if start == goal {
        return Some(Path {
            cells: Vec::new(),
            cost: 0,
        });
    }
    let enterable = |cell: CellPos| grid.is_passable(cell) && !blocked(cell);
    if !enterable(goal) {
        return None;
    }
    let cells = (grid.width() as usize) * (grid.height() as usize);
    let mut cost = vec![u32::MAX; cells];
    let mut parent = vec![usize::MAX; cells];
    let mut closed = vec![false; cells];
    let mut open = BinaryHeap::new();
    let start_index = cell_index(grid, start);
    let goal_index = cell_index(grid, goal);
    cost[start_index] = 0;
    let h = octile_distance(start, goal);
    open.push(Reverse((h, h, start_index)));
    while let Some(Reverse((_, _, index))) = open.pop() {
        if std::mem::replace(&mut closed[index], true) {
            continue;
        }
        if index == goal_index {
            let mut path = Vec::new();
            let mut at = index;
            while at != start_index {
                path.push(cell_at(grid, at));
                at = parent[at];
            }
            path.reverse();
            return Some(Path {
                cells: path,
                cost: cost[goal_index],
            });
        }
        let here = cell_at(grid, index);
        for (next, step_cost) in neighbours(grid, here, &enterable) {
            let next_index = cell_index(grid, next);
            let next_cost = cost[index].saturating_add(step_cost);
            if !closed[next_index] && next_cost < cost[next_index] {
                cost[next_index] = next_cost;
                parent[next_index] = index;
                let h = octile_distance(next, goal);
                open.push(Reverse((next_cost.saturating_add(h), h, next_index)));
            }
        }
    }
    None
}

/// Up to `count` distinct destination cells around `target`: the passable cells that are not
/// `excluded`, in order of path cost from `target` over passable terrain (ties by row, then
/// column). The search starts at `target` even if it is impassable, so a target on a rock
/// yields the passable cells around it; cells cut off from the target by impassable terrain
/// are never returned.
pub fn destination_cells(
    grid: &Grid,
    target: CellPos,
    count: usize,
    excluded: impl Fn(CellPos) -> bool,
) -> Vec<CellPos> {
    let mut found = Vec::new();
    if !grid.contains(target) || count == 0 {
        return found;
    }
    let passable = |cell: CellPos| grid.is_passable(cell);
    let cells = (grid.width() as usize) * (grid.height() as usize);
    let mut cost = vec![u32::MAX; cells];
    let mut closed = vec![false; cells];
    let mut open = BinaryHeap::new();
    cost[cell_index(grid, target)] = 0;
    open.push(Reverse((0_u32, target.y, target.x)));
    while let Some(Reverse((here_cost, y, x))) = open.pop() {
        let here = CellPos::new(x, y);
        let index = cell_index(grid, here);
        if std::mem::replace(&mut closed[index], true) {
            continue;
        }
        if passable(here) && !excluded(here) {
            found.push(here);
            if found.len() == count {
                break;
            }
        }
        for (next, step_cost) in neighbours(grid, here, &passable) {
            let next_index = cell_index(grid, next);
            let next_cost = here_cost.saturating_add(step_cost);
            if !closed[next_index] && next_cost < cost[next_index] {
                cost[next_index] = next_cost;
                open.push(Reverse((next_cost, next.y, next.x)));
            }
        }
    }
    found
}

/// The passable cell nearest to `start` by path cost over passable terrain (ties by row, then
/// column) for which `accept` holds, or `None` if no such cell is reachable. `start` itself is
/// considered first.
pub(crate) fn nearest_cell(
    grid: &Grid,
    start: CellPos,
    accept: impl Fn(CellPos) -> bool,
) -> Option<CellPos> {
    destination_cells(grid, start, 1, |cell| !accept(cell))
        .first()
        .copied()
}

type PathQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static SimId,
        &'static Mobile,
        &'static mut Orders,
        &'static Owner,
        &'static Unit,
    ),
    With<Unit>,
>;

/// A unit without an order standing at a cell centre. It stays where it is until it is given an
/// order, so the cell it holds is a lasting obstacle.
fn is_idle(mobile: &Mobile, orders: &Orders) -> bool {
    mobile.step.is_none() && orders.current.is_none()
}

/// A unit that is not advancing: idle, waiting at a cell centre for a blocked cell, or waiting
/// there for a path.
fn is_waiting(mobile: &Mobile, orders: &Orders) -> bool {
    mobile.step.is_none()
        && orders.current.as_ref().is_none_or(|order| {
            order.blocked_ticks > 0 || matches!(order.state, PathState::Requested { .. })
        })
}

/// Serves up to [`PATH_REQUESTS_PER_TICK`] path requests, oldest first (then by unit id).
///
/// A path starts at the cell whose centre the unit reaches next. If no path over terrain leads
/// to the destination, the order ends with [`MoveFailure::Unreachable`]. Otherwise a request
/// without `avoid_units` (a new order, or terrain that changed) takes the cheapest path over
/// terrain, and a request with `avoid_units` (the unit waited for a cell held by another unit)
/// takes, in this order of preference:
///
/// 1. the cheapest path around all waiting units (idle, or themselves waiting at a centre);
/// 2. else, if the units it waits for wait in turn for it (a wait-for cycle, such as two units
///    that each want the other's cell): if it has the highest id among the members of the cycle
///    that have a free neighbouring cell, a step aside to the free neighbouring cell nearest to
///    its destination, after which it requests a new path (the others of the cycle keep
///    waiting, so exactly one unit gives way); if no member can step aside, the other members
///    count as lasting obstacles, like idle units, in the steps below;
/// 3. else the cheapest path around lasting obstacles (idle units) only, queueing behind units
///    that wait in a crowd and will move on;
/// 4. else, since lasting obstacles close every way to the destination (for example, group
///    members that arrived first surround it, or an idle unit stands on it), the unit takes as
///    its new destination the cell nearest to the old one that it can reach around them
///    ([`nearest_reachable`]), possibly the cell it stands in.
///
/// Every path served counts toward the order's limit of paths without progress
/// (`movement::MAX_PATHS_WITHOUT_PROGRESS`), except one asked for while the unit waited behind
/// units that are moving on that only confirms the way it already follows (docs/DECISIONS.md
/// D22); a path that leads to the destination records its cost as the order's remaining cost
/// (docs/DECISIONS.md D21).
pub(crate) fn find_paths(
    grid: Res<Grid>,
    mut events: ResMut<Messages<SimEvent>>,
    players: Res<Players>,
    definitions: Res<Definitions>,
    mut units: PathQuery,
) {
    let mut requests: Vec<(u64, SimId, Entity)> = units
        .iter()
        .filter_map(
            |(entity, id, _, orders, ..)| match orders.current.as_ref()?.state {
                PathState::Requested { since, .. } => Some((since, *id, entity)),
                PathState::Following => None,
            },
        )
        .collect();
    if requests.is_empty() {
        return;
    }
    requests.sort_unstable();
    requests.truncate(PATH_REQUESTS_PER_TICK);
    let collect = |filter: fn(&Mobile, &Orders) -> bool| -> BTreeSet<SimId> {
        units
            .iter()
            .filter(|(_, _, mobile, orders, ..)| filter(mobile, orders))
            .map(|(_, id, ..)| *id)
            .collect()
    };
    let waiting = collect(is_waiting);
    let idle = collect(is_idle);
    // Each waiting unit with an order: the cell it stands in, the cell it wants to enter next,
    // and its destination.
    let wants: BTreeMap<SimId, Waiter> = units
        .iter()
        .filter(|(_, _, mobile, orders, ..)| is_waiting(mobile, orders))
        .filter_map(|(_, id, mobile, orders, ..)| {
            let order = orders.current.as_ref()?;
            Some((
                *id,
                Waiter {
                    cell: mobile.cell,
                    next: *order.path.last()?,
                    destination: order.destination,
                },
            ))
        })
        .collect();
    let crushable = Crushable::of(
        &definitions,
        units
            .iter()
            .map(|(entity, id, _, _, owner, unit)| (entity, *id, owner.0, unit.kind)),
    );

    for (_, id, entity) in requests {
        let on_foot = grid.is_on_foot(id);
        let grid = grid.navigation_for(grid.domain_of(id));
        let Ok((_, _, mobile, mut orders, owner, _)) = units.get_mut(entity) else {
            continue;
        };
        let start = mobile.next_centre();
        let Some(order) = orders.current.as_mut() else {
            continue;
        };
        let PathState::Requested {
            avoid_units,
            queued,
            ..
        } = order.state
        else {
            continue;
        };
        // Infantry the unit would drive over on this order is not in its way.
        let crush = order.crush;
        let crushes = |holder: SimId| crush && crushable.by(&players, owner.0, holder);
        let held_by = |obstacles: &BTreeSet<SimId>, cell: CellPos| {
            grid.blocked(cell, id, on_foot, |holder| {
                obstacles.contains(&holder) && !crushes(holder)
            })
        };
        let around = |obstacles: &BTreeSet<SimId>| {
            find_path(&grid, start, order.destination, |cell| {
                held_by(obstacles, cell)
            })
        };
        let mut lasting = idle.clone();
        let path = if avoid_units {
            around(&waiting).or_else(|| match wait_cycle(&grid, id, &wants, &waiting) {
                Some(cycle) => match cycle.yielder {
                    Some(yielder) if yielder == id => {
                        step_aside(&grid, id, start, order.destination)
                    }
                    Some(_) => around(&lasting),
                    None => {
                        lasting.extend(cycle.members);
                        around(&lasting)
                    }
                },
                None => around(&lasting),
            })
        } else {
            None
        };
        let path = match path {
            Some(path) => path,
            None => match find_path(&grid, start, order.destination, |_| false) {
                None => {
                    orders.current = None;
                    events.write(SimEvent::MoveFailed {
                        unit: id,
                        reason: MoveFailure::Unreachable,
                    });
                    continue;
                }
                Some(over_terrain) if !avoid_units => over_terrain,
                Some(_) => {
                    let (destination, path) =
                        nearest_reachable(&grid, start, order.destination, |cell| {
                            held_by(&lasting, cell)
                        });
                    order.destination = destination;
                    order.retargets = order.retargets.saturating_add(1);
                    path
                }
            },
        };
        // A step aside leads to a neighbour, not to the destination: following it is no
        // progress (docs/DECISIONS.md D21).
        let leads_to_destination = path.cells.last().copied().unwrap_or(start) == order.destination;
        order.remaining = leads_to_destination.then_some(path.cost);
        let mut cells = path.cells;
        cells.reverse();
        // A unit that waits behind units that are moving on asks for a way around them every
        // `REPATH_AFTER_BLOCKED_TICKS` ticks however long the wait lasts (up to 362 ticks for a
        // step at speed 1). A path that only confirms the way it already follows is then not a
        // new path, so the wait does not use up its paths (docs/DECISIONS.md D22).
        if !(queued && cells == order.path) {
            order.paths_since_progress = order.paths_since_progress.saturating_add(1);
        }
        order.path = cells;
        order.state = PathState::Following;
    }
}

/// A unit waiting at the centre of `cell` to enter `next`.
struct Waiter {
    cell: CellPos,
    next: CellPos,
    destination: CellPos,
}

/// A wait-for cycle through a unit, and the member that gives way.
struct WaitCycle {
    members: BTreeSet<SimId>,
    /// The member with the highest id among those that can step aside, if any.
    yielder: Option<SimId>,
}

/// The wait-for cycle through `unit`, if following, from `unit`, the waiting unit that holds the
/// cell each unit wants to enter leads back to `unit`. The chain visits each unit at most once,
/// and every member of a cycle computes the same cycle and yielder from the same state. Of the
/// units that keep a unit out of a cell (several, where infantry hold it) the chain follows the
/// first.
fn wait_cycle(
    grid: &Grid,
    unit: SimId,
    wants: &BTreeMap<SimId, Waiter>,
    waiting: &BTreeSet<SimId>,
) -> Option<WaitCycle> {
    let mut members = BTreeSet::new();
    let mut at = unit;
    while members.insert(at) {
        let holder = wants.get(&at).and_then(|waiter| {
            let blockers = grid.blockers(waiter.next, at, grid.is_on_foot(at));
            blockers.into_iter().flatten().next()
        })?;
        if holder == unit {
            let yielder = members
                .iter()
                .copied()
                .filter(|member| {
                    wants.get(member).is_some_and(|waiter| {
                        step_aside(grid, *member, waiter.cell, waiter.destination).is_some()
                    })
                })
                .max();
            return Some(WaitCycle { members, yielder });
        }
        if !waiting.contains(&holder) {
            return None;
        }
        at = holder;
    }
    None
}

/// A one-step path for `unit` to the passable neighbour of `start` with room for it nearest to
/// `destination` (by octile distance, then row and column) that a step may reach under the
/// diagonal rule of [`find_path`]; `None` if every neighbour is taken.
fn step_aside(grid: &Grid, unit: SimId, start: CellPos, destination: CellPos) -> Option<Path> {
    let on_foot = grid.is_on_foot(unit);
    neighbours(grid, start, &|cell| {
        grid.is_passable(cell) && grid.has_room(cell, on_foot)
    })
    .min_by_key(|&(cell, _)| (octile_distance(cell, destination), cell.y, cell.x))
    .map(|(cell, cost)| Path {
        cells: vec![cell],
        cost,
    })
}

/// The cell nearest to `goal` (by octile distance, then path cost, then row and column) among
/// the cells reachable from `start` over cells that are passable and not `blocked`, and the
/// cheapest path to it. `start` itself is always reachable.
pub fn nearest_reachable(
    grid: &Grid,
    start: CellPos,
    goal: CellPos,
    blocked: impl Fn(CellPos) -> bool,
) -> (CellPos, Path) {
    let empty = Path {
        cells: Vec::new(),
        cost: 0,
    };
    if !grid.contains(start) {
        return (start, empty);
    }
    let enterable = |cell: CellPos| grid.is_passable(cell) && !blocked(cell);
    let cells = (grid.width() as usize) * (grid.height() as usize);
    let mut cost = vec![u32::MAX; cells];
    let mut parent = vec![usize::MAX; cells];
    let mut closed = vec![false; cells];
    let mut open = BinaryHeap::new();
    let start_index = cell_index(grid, start);
    cost[start_index] = 0;
    open.push(Reverse((0_u32, start_index)));
    let mut best = (
        octile_distance(start, goal),
        0_u32,
        start.y,
        start.x,
        start_index,
    );
    while let Some(Reverse((here_cost, index))) = open.pop() {
        if std::mem::replace(&mut closed[index], true) {
            continue;
        }
        let here = cell_at(grid, index);
        let candidate = (
            octile_distance(here, goal),
            here_cost,
            here.y,
            here.x,
            index,
        );
        if candidate < best {
            best = candidate;
        }
        for (next, step_cost) in neighbours(grid, here, &enterable) {
            let next_index = cell_index(grid, next);
            let next_cost = here_cost.saturating_add(step_cost);
            if !closed[next_index] && next_cost < cost[next_index] {
                cost[next_index] = next_cost;
                parent[next_index] = index;
                open.push(Reverse((next_cost, next_index)));
            }
        }
    }
    let (_, best_cost, _, _, best_index) = best;
    let mut path = Vec::new();
    let mut at = best_index;
    while at != start_index {
        path.push(cell_at(grid, at));
        at = parent[at];
    }
    path.reverse();
    (
        cell_at(grid, best_index),
        Path {
            cells: path,
            cost: best_cost,
        },
    )
}
