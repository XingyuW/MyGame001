//! Maps that grow with the number of players (docs/DECISIONS.md, "Ten players on maps that grow
//! with them").
//!
//! A map file has a fixed size and lists a few start positions. A match with more than two
//! players is played on an enlarged copy of the map, made by [`enlarged_map`] before match setup:
//!
//! * **Size.** Every player beyond the second adds an eighth of the map's size
//!   ([`SCALE_DIVISOR`]): two players play on the map as its file defines it, ten on one twice as
//!   wide and high ([`enlarged_size`]). No side grows beyond [`MAX_MAP_SIDE`].
//! * **Terrain.** Every cell of the enlarged map shows a cell of the map file (nearest
//!   neighbour): each column and row of the file is repeated one or more times, in order
//!   ([`axis_map`]). So whatever was connected stays connected, over land and over water, bases
//!   keep at least their clear ground, and a point-symmetric map stays point-symmetric.
//! * **Start positions.** The start positions of the file move with their terrain. A match with
//!   more players than the file has start positions gets the missing ones made here: on ground
//!   that is reachable from the first start, as far from every other start as possible, in
//!   point-mirrored pairs where the mirrored cell qualifies too, each with cleared building
//!   ground and an ore field of its own ([`PAD_RADIUS`], [`ORE_DISTANCE`]).
//!
//! Everything is integer arithmetic on the map alone (no clock, no random numbers, no hash map),
//! so the same map and player count give the same result on every machine (invariants I2, I4).
//! The only terrain that changes is at the made start positions, and there cells only ever become
//! passable: no land route is cut, and unless a map leaves no other choice (the relaxed tier of
//! `make_starts`) no water is filled in.

use std::collections::VecDeque;
use std::fmt;

use crate::map::{Cell, CellPos, Grid, MAX_MAP_SIDE};
use crate::skirmish_defs::{MAX_MATCH_PLAYERS, MIN_MATCH_PLAYERS};

/// Every player beyond the second adds one part in this many to the map's width and height.
pub const SCALE_DIVISOR: u32 = 8;

/// The centre of the 3 × 3 construction yard a player starts with lies this many cells right of
/// and below its start position (the origin of the yard's footprint). Start positions are moved
/// by their yard's centre, about which the shipped maps mirror them.
pub const YARD_CENTRE_OFFSET: i32 = 1;

/// A made start position clears the ground this many cells around its yard's centre (Chebyshev),
/// less the corners ([`PAD_CORNER`]): the yard, the starting units beside it, and room for a base.
pub const PAD_RADIUS: i32 = 6;

/// Cells of the pad lie at most this far from the yard's centre by Manhattan distance, which
/// cuts the corners of the square off.
pub const PAD_CORNER: i32 = 9;

/// The cells this near the yard's centre (Chebyshev) always become building ground, whatever they
/// were: the yard and the ring its starting units stand on.
pub const CORE_RADIUS: i32 = 3;

/// No ore is planted this near a yard's centre (Chebyshev): the yard's apron stays building
/// ground.
pub const APRON_RADIUS: i32 = 5;

/// The centre of a made start position's ore field lies about this far from the yard's centre
/// (Euclidean: from this distance to [`ORE_DISTANCE`]` + 1` cells, exclusive).
pub const ORE_DISTANCE: i32 = 9;

/// Radius of a made ore field: the cells whose squared distance from its centre is at most
/// `r * r + r`, the disc the map generators paint.
pub const ORE_RADIUS: i32 = 3;

/// Fewest cells of ore a made start position has; a site that cannot hold as many is not used.
pub const MIN_ORE_CELLS: usize = 20;

/// Ore of a planted cell on a map without any ore to copy the amount from.
pub const DEFAULT_ORE: u32 = 500;

/// Smallest distance in cells between the yard centres of a made start position and any other
/// start position: their pads and ore fields never touch.
pub const MIN_START_DISTANCE: i32 = 22;

/// Smallest distance from a made yard's centre to the edge of the map.
pub const EDGE_MARGIN: i32 = PAD_RADIUS + 2;

/// Most sites tried per tier for one start position before the next tier is used.
pub const MAX_SITE_ATTEMPTS: usize = 256;

/// The low four bits of a look are its ground, the high four the feature standing on it
/// (`LoadedMap::looks`; the game's `battlefield::looks` gives the values their names, and its
/// tests check that the ones below agree with it).
pub const LOOK_GROUND_MASK: u8 = 15;

/// The ground "derived from the terrain": a map without looks. Made ground keeps look 0 there.
pub const LOOK_GROUND_AUTO: u8 = 0;

/// The ground of water and of bridge decks.
pub const LOOK_GROUND_WATER: u8 = 6;

/// The feature of an ore cell.
pub const LOOK_FEATURE_ORE: u8 = 1;

/// The feature of a bridge deck, which no made start position touches.
pub const LOOK_FEATURE_BRIDGE: u8 = 8;

/// The feature of a cell of gems (RA5): a cell with ore and this feature holds gems
/// ([`Grid::is_gems`]).
pub const LOOK_FEATURE_GEMS: u8 = 10;

/// A map enlarged for a match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnlargedMap {
    /// The terrain, without units or buildings.
    pub grid: Grid,
    /// The look of every cell, in row-major order.
    pub looks: Vec<u8>,
    /// The start positions: those of the map file in their order, moved with their terrain, then
    /// the made ones. At least as many as the match has players.
    pub starts: Vec<CellPos>,
}

/// Why a map could not be enlarged for a match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapScaleError {
    /// The map has no ground left for start position `start` of a match with `players` players:
    /// no cell reachable from the first start is far enough from the edge and from every other
    /// start, on mostly dry ground, with room for an ore field.
    NoStartSite { start: usize, players: usize },
    /// The enlarged map would be larger than a map may be. [`enlarged_size`] never asks for one;
    /// reported instead of assumed.
    TooLarge { width: u32, height: u32 },
}

impl fmt::Display for MapScaleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoStartSite { start, players } => write!(
                f,
                "the map has no room for start position {start} of a match with {players} \
                 players: no ground reachable from the first start lies at least \
                 {MIN_START_DISTANCE} cells from every other start and {EDGE_MARGIN} from the \
                 edge, mostly dry, with room for an ore field"
            ),
            Self::TooLarge { width, height } => write!(
                f,
                "the enlarged map of {width} x {height} cells exceeds the maximum side of \
                 {MAX_MAP_SIDE} cells"
            ),
        }
    }
}

impl std::error::Error for MapScaleError {}

/// `side` cells grown by `steps` parts in [`SCALE_DIVISOR`], rounded up to an even number of
/// added cells: half of them on either side of the middle, so that the middle of the map stays
/// its middle and a point-symmetric map can stay point-symmetric.
fn grown(side: u32, steps: u32) -> u64 {
    let added = (u64::from(side) * u64::from(steps)).div_ceil(2 * u64::from(SCALE_DIVISOR));
    u64::from(side) + 2 * added
}

/// The size of the map of a match with `players` players on a map file of `width` × `height`
/// cells: both sides grown by `players − 2` parts in [`SCALE_DIVISOR`] (two players: unchanged; ten:
/// doubled), the added cells of a side rounded up to an even number. Where that would exceed
/// [`MAX_MAP_SIDE`] on either side, both sides grow by the most parts that fit, so the map keeps
/// its proportions. More than [`MAX_MATCH_PLAYERS`] players count as that many.
pub fn enlarged_size(width: u32, height: u32, players: usize) -> (u32, u32) {
    let most = MAX_MATCH_PLAYERS - MIN_MATCH_PLAYERS;
    let steps = u32::try_from(players.saturating_sub(MIN_MATCH_PLAYERS).min(most)).unwrap_or(0);
    let limit = u64::from(MAX_MAP_SIDE);
    (0..=steps)
        .rev()
        .map(|steps| (grown(width, steps), grown(height, steps)))
        .find(|&(wide, high)| wide <= limit && high <= limit)
        .and_then(|(wide, high)| Some((u32::try_from(wide).ok()?, u32::try_from(high).ok()?)))
        .unwrap_or((width, height))
}

/// For every column (or row) of a side enlarged from `source` to `target` cells, the column of
/// the map file it shows. `target − source` is even and not negative ([`enlarged_size`]).
///
/// The lower half samples the file at its cells' centres, `⌊(2t + 1) · source / (2 · target)⌋`,
/// and the upper half mirrors the lower one, so the map is symmetric about its middle:
/// `map[target − 1 − t] = source − 1 − map[t]`. It starts at 0, ends at `source − 1`, and never
/// steps by more than one, so every column of the file is shown at least once, in order.
pub fn axis_map(source: u32, target: u32) -> Vec<u32> {
    let mut map = vec![0; target as usize];
    for lower in 0..target.div_ceil(2) {
        let sample = (2 * u64::from(lower) + 1) * u64::from(source) / (2 * u64::from(target));
        let sample = u32::try_from(sample)
            .unwrap_or(0)
            .min(source.saturating_sub(1));
        let upper = target - 1 - lower;
        map[lower as usize] = sample;
        map[upper as usize] = source.saturating_sub(1).saturating_sub(sample);
    }
    map
}

/// Where column `at` of the map file lies on a side enlarged with `map` ([`axis_map`]): the
/// outermost of the columns that show it, so that mirrored columns of the file stay mirrored;
/// the middle column of an odd side stays the middle. Columns outside the file count as its
/// nearest one.
fn moved(map: &[u32], source: u32, at: i32) -> i32 {
    let last = i32::try_from(source).unwrap_or(i32::MAX).saturating_sub(1);
    let at = at.clamp(0, last.max(0));
    let first = |column: i32| {
        map.iter()
            .position(|&shown| i64::from(shown) == i64::from(column))
            .and_then(|index| i32::try_from(index).ok())
            .unwrap_or(0)
    };
    let end = i32::try_from(map.len()).unwrap_or(i32::MAX) - 1;
    match (2 * i64::from(at) + 1).cmp(&i64::from(source)) {
        std::cmp::Ordering::Less => first(at),
        std::cmp::Ordering::Equal => end / 2,
        std::cmp::Ordering::Greater => end - first(last - at),
    }
}

/// The map being made: terrain and looks of every cell, row-major.
struct Canvas {
    width: i32,
    height: i32,
    cells: Vec<Cell>,
    looks: Vec<u8>,
}

/// The four neighbours of a cell. Passable cells joined by them are the cells units can walk
/// between: the pathfinder's diagonal steps never pass between two blocked orthogonal neighbours.
const NEIGHBOURS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

impl Canvas {
    fn index(&self, x: i32, y: i32) -> Option<usize> {
        ((0..self.width).contains(&x) && (0..self.height).contains(&y))
            .then(|| (y * self.width + x) as usize)
    }

    fn cell(&self, x: i32, y: i32) -> Option<&Cell> {
        self.index(x, y).and_then(|index| self.cells.get(index))
    }

    fn passable(&self, x: i32, y: i32) -> bool {
        self.cell(x, y).is_some_and(|cell| cell.passable)
    }

    fn look(&self, x: i32, y: i32) -> u8 {
        self.index(x, y)
            .and_then(|index| self.looks.get(index))
            .copied()
            .unwrap_or(0)
    }

    fn is_bridge(&self, x: i32, y: i32) -> bool {
        self.look(x, y) >> 4 == LOOK_FEATURE_BRIDGE
    }

    /// Whether each cell can be walked to from `from` over passable terrain.
    fn reachable(&self, from: (i32, i32)) -> Vec<bool> {
        let mut reached = vec![false; self.cells.len()];
        let mut queue = VecDeque::new();
        if let Some(index) = self.index(from.0, from.1)
            && self.passable(from.0, from.1)
        {
            reached[index] = true;
            queue.push_back(from);
        }
        while let Some((x, y)) = queue.pop_front() {
            for (dx, dy) in NEIGHBOURS {
                let next = (x + dx, y + dy);
                if let Some(index) = self.index(next.0, next.1)
                    && self.passable(next.0, next.1)
                    && !std::mem::replace(&mut reached[index], true)
                {
                    queue.push_back(next);
                }
            }
        }
        reached
    }
}

/// How many cells with a property a rectangle of the canvas holds, from sums over the rectangles
/// that start at the first cell.
struct Counts {
    stride: usize,
    sums: Vec<u32>,
}

impl Counts {
    /// Counts the cells for whose row-major index `counted` holds.
    fn of(canvas: &Canvas, counted: impl Fn(usize) -> bool) -> Self {
        let (width, height) = (canvas.width as usize, canvas.height as usize);
        let stride = width + 1;
        let mut sums = vec![0_u32; stride * (height + 1)];
        for y in 0..height {
            let mut row = 0;
            for x in 0..width {
                row += u32::from(counted(y * width + x));
                sums[(y + 1) * stride + x + 1] = sums[y * stride + x + 1] + row;
            }
        }
        Self { stride, sums }
    }

    /// The count in the square of `radius` cells around (`x`, `y`), which lies on the canvas.
    fn around(&self, x: i32, y: i32, radius: i32) -> u32 {
        let (x0, y0) = ((x - radius) as usize, (y - radius) as usize);
        let (x1, y1) = ((x + radius + 1) as usize, (y + radius + 1) as usize);
        let at = |x: usize, y: usize| self.sums[y * self.stride + x];
        at(x1, y1) + at(x0, y0) - at(x1, y0) - at(x0, y1)
    }
}

/// How much a site for a made start position may change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tier {
    /// The yard's ground holds neither water nor ore, at most a third of the square around it
    /// is water, half of it is open ground already, and no bridge leads into it: nothing but
    /// scenery and rock is cleared.
    Strict,
    /// At most half of the square around the yard is water; water, ore and bridge decks under the
    /// yard and the ring around it are filled in. Only for maps that have no strict site left.
    Relaxed,
}

/// Squared distance between two cells.
fn distance_squared(a: (i32, i32), b: (i32, i32)) -> i64 {
    let (dx, dy) = (i64::from(a.0 - b.0), i64::from(a.1 - b.1));
    dx * dx + dy * dy
}

/// What making a start position at a site changes: the cells that become open building ground,
/// and the cells that become ore.
struct Site {
    centre: (i32, i32),
    ground: Vec<(i32, i32)>,
    ore: Vec<(i32, i32)>,
    /// The ground look of filled-in water: the commonest ground of the pad's dry cells.
    fill: u8,
}

/// The offsets of the cells of a made start position's pad from its yard's centre.
fn pad_offsets() -> impl Iterator<Item = (i32, i32)> {
    (-PAD_RADIUS..=PAD_RADIUS)
        .flat_map(|dy| (-PAD_RADIUS..=PAD_RADIUS).map(move |dx| (dx, dy)))
        .filter(|(dx, dy)| dx.abs() + dy.abs() <= PAD_CORNER)
}

/// Plans a start position with its yard's centre at `centre`, given the yard centres of all
/// other starts (`others`): the cells of the pad that become open ground (those that can be
/// walked to from the centre once cleared; water beyond the core stays, and so does ore and any
/// bridge), and the ore field. `flipped` mirrors the order in which places for the ore field are
/// tried, so that the mirrored site of a pair gets the mirrored field. `None` if no ore field of
/// [`MIN_ORE_CELLS`] cells fits.
fn plan_site(
    canvas: &Canvas,
    centre: (i32, i32),
    others: &[(i32, i32)],
    flipped: bool,
) -> Option<Site> {
    let side = (2 * PAD_RADIUS + 1) as usize;
    let slot = |dx: i32, dy: i32| ((dy + PAD_RADIUS) as usize) * side + (dx + PAD_RADIUS) as usize;
    let in_core = |dx: i32, dy: i32| dx.abs() <= CORE_RADIUS && dy.abs() <= CORE_RADIUS;
    // The cells of the pad that may be cleared, and those that can be walked over as they are.
    let mut clearable = vec![false; side * side];
    let mut walkable = vec![false; side * side];
    let mut grounds = [0_u32; LOOK_GROUND_MASK as usize + 1];
    for (dx, dy) in pad_offsets() {
        let (x, y) = (centre.0 + dx, centre.1 + dy);
        let Some(cell) = canvas.cell(x, y) else {
            continue;
        };
        let bridge = canvas.is_bridge(x, y);
        clearable[slot(dx, dy)] = in_core(dx, dy) || (!cell.water && cell.ore == 0 && !bridge);
        walkable[slot(dx, dy)] = cell.passable;
        let ground = canvas.look(x, y) & LOOK_GROUND_MASK;
        if !cell.water && ground != LOOK_GROUND_WATER {
            grounds[usize::from(ground)] += 1;
        }
    }
    let fill = (0..=LOOK_GROUND_MASK)
        .max_by_key(|&ground| (grounds[usize::from(ground)], std::cmp::Reverse(ground)))
        .filter(|&ground| grounds[usize::from(ground)] > 0)
        .unwrap_or(LOOK_GROUND_AUTO);
    // Cleared ground must join the centre: a cleared pocket beyond water would be building
    // ground no unit can reach.
    let mut joined = vec![false; side * side];
    let mut queue = VecDeque::from([(0, 0)]);
    joined[slot(0, 0)] = true;
    while let Some((dx, dy)) = queue.pop_front() {
        for (sx, sy) in NEIGHBOURS {
            let (nx, ny) = (dx + sx, dy + sy);
            if nx.abs() > PAD_RADIUS || ny.abs() > PAD_RADIUS || nx.abs() + ny.abs() > PAD_CORNER {
                continue;
            }
            let next = slot(nx, ny);
            if (clearable[next] || walkable[next]) && !std::mem::replace(&mut joined[next], true) {
                queue.push_back((nx, ny));
            }
        }
    }
    let ground: Vec<(i32, i32)> = pad_offsets()
        .filter(|&(dx, dy)| joined[slot(dx, dy)] && clearable[slot(dx, dy)])
        .map(|(dx, dy)| (centre.0 + dx, centre.1 + dy))
        .collect();

    // The ore field: the place around the yard where most of its disc can be planted and walked
    // to from the pad.
    let reach = ORE_DISTANCE + 1 + ORE_RADIUS + 1;
    let box_side = (2 * reach + 1) as usize;
    let box_slot = |x: i32, y: i32| {
        let (dx, dy) = (x - centre.0, y - centre.1);
        (dx.abs() <= reach && dy.abs() <= reach)
            .then(|| ((dy + reach) as usize) * box_side + (dx + reach) as usize)
    };
    let cleared = |x: i32, y: i32| {
        let (dx, dy) = (x - centre.0, y - centre.1);
        dx.abs() <= PAD_RADIUS
            && dy.abs() <= PAD_RADIUS
            && dx.abs() + dy.abs() <= PAD_CORNER
            && joined[slot(dx, dy)]
            && clearable[slot(dx, dy)]
    };
    let open = |x: i32, y: i32| canvas.passable(x, y) || cleared(x, y);
    // Cells around the yard that units can walk to from it once the pad is cleared.
    let mut base = vec![false; box_side * box_side];
    let mut queue = VecDeque::from([centre]);
    if let Some(index) = box_slot(centre.0, centre.1) {
        base[index] = true;
    }
    while let Some((x, y)) = queue.pop_front() {
        for (sx, sy) in NEIGHBOURS {
            let next = (x + sx, y + sy);
            if let Some(index) = box_slot(next.0, next.1)
                && open(next.0, next.1)
                && !std::mem::replace(&mut base[index], true)
            {
                queue.push_back(next);
            }
        }
    }
    let plantable = |x: i32, y: i32| {
        canvas
            .cell(x, y)
            .is_some_and(|cell| !cell.water && !canvas.is_bridge(x, y))
            && std::iter::once(&centre)
                .chain(others)
                .all(|&(cx, cy)| (x - cx).abs() > APRON_RADIUS || (y - cy).abs() > APRON_RADIUS)
    };
    let near = i64::from(ORE_DISTANCE) * i64::from(ORE_DISTANCE);
    let far = i64::from(ORE_DISTANCE + 1) * i64::from(ORE_DISTANCE + 1);
    let mut places: Vec<(i32, i32)> = (-ORE_DISTANCE - 1..=ORE_DISTANCE + 1)
        .flat_map(|dy| (-ORE_DISTANCE - 1..=ORE_DISTANCE + 1).map(move |dx| (dx, dy)))
        .filter(|&offset| (near..far).contains(&distance_squared(offset, (0, 0))))
        .collect();
    if flipped {
        places.reverse();
    }
    let disc = i64::from(ORE_RADIUS) * i64::from(ORE_RADIUS) + i64::from(ORE_RADIUS);
    let mut best: Vec<(i32, i32)> = Vec::new();
    for (ox, oy) in places {
        let middle = (centre.0 + ox, centre.1 + oy);
        let field: Vec<(i32, i32)> = (-ORE_RADIUS - 1..=ORE_RADIUS + 1)
            .flat_map(|dy| (-ORE_RADIUS - 1..=ORE_RADIUS + 1).map(move |dx| (dx, dy)))
            .filter(|&offset| distance_squared(offset, (0, 0)) <= disc)
            .map(|(dx, dy)| (middle.0 + dx, middle.1 + dy))
            .filter(|&(x, y)| plantable(x, y))
            .collect();
        if field.len() <= best.len() {
            continue;
        }
        // Of the field, the cells a harvester can walk to: those on or beside ground joined to
        // the yard, and the cells of the field joined to them.
        let touches = |&(x, y): &(i32, i32)| {
            std::iter::once((0, 0))
                .chain(NEIGHBOURS)
                .any(|(sx, sy)| box_slot(x + sx, y + sy).is_some_and(|index| base[index]))
        };
        let mut planted: Vec<(i32, i32)> = field.iter().copied().filter(touches).collect();
        let mut next = 0;
        while next < planted.len() {
            let (x, y) = planted[next];
            next += 1;
            for (sx, sy) in NEIGHBOURS {
                let beside = (x + sx, y + sy);
                if field.contains(&beside) && !planted.contains(&beside) {
                    planted.push(beside);
                }
            }
        }
        if planted.len() > best.len() {
            best = planted;
        }
    }
    (best.len() >= MIN_ORE_CELLS).then(|| {
        best.sort_unstable_by_key(|&(x, y)| (y, x));
        Site {
            centre,
            ground,
            ore: best,
            fill,
        }
    })
}

/// Makes the start position planned as `site`: its pad becomes open building ground that keeps
/// each cell's ground look, and its ore field ore of `amount` per cell on the same ground.
fn make_site(canvas: &mut Canvas, site: &Site, amount: u32) {
    let ground_of = |look: u8| match look & LOOK_GROUND_MASK {
        LOOK_GROUND_WATER => site.fill,
        ground => ground,
    };
    for &(x, y) in &site.ground {
        let Some(index) = canvas.index(x, y) else {
            continue;
        };
        canvas.cells[index] = Cell {
            passable: true,
            water: false,
            buildable: true,
            ore: 0,
        };
        canvas.looks[index] = ground_of(canvas.looks[index]);
    }
    for &(x, y) in &site.ore {
        let Some(index) = canvas.index(x, y) else {
            continue;
        };
        if canvas.cells[index].ore > 0 {
            // Ore that was there already stays as it is.
            continue;
        }
        canvas.cells[index] = Cell {
            passable: true,
            water: false,
            buildable: false,
            ore: amount,
        };
        canvas.looks[index] = match ground_of(canvas.looks[index]) {
            LOOK_GROUND_AUTO => LOOK_GROUND_AUTO,
            ground => ground | LOOK_FEATURE_ORE << 4,
        };
    }
}

/// Adds made start positions to `centres` (the yard centres of the starts so far, the first
/// start's first) until there are `players` of them, and makes their pads and ore fields on
/// `canvas`.
///
/// Sites are chosen farthest first: of the cells that qualify, the one whose nearest start is
/// farthest away (squared distance, then the first in row-major order). While two or more are
/// missing, a site is chosen together with its point mirror, if that qualifies too and the two
/// are far enough apart, so that a point-symmetric map gets equal bases. A chosen site that has
/// no room for an ore field is passed over.
fn make_starts(
    canvas: &mut Canvas,
    centres: &mut Vec<(i32, i32)>,
    players: usize,
) -> Result<(), MapScaleError> {
    let amount = canvas
        .cells
        .iter()
        .map(|cell| cell.ore)
        .max()
        .filter(|&ore| ore > 0)
        .unwrap_or(DEFAULT_ORE);
    let first = centres.first().map_or((0, 0), |&(x, y)| {
        (x - YARD_CENTRE_OFFSET, y - YARD_CENTRE_OFFSET)
    });
    let least = i64::from(MIN_START_DISTANCE) * i64::from(MIN_START_DISTANCE);
    let area = (2 * PAD_RADIUS + 1) as u32 * (2 * PAD_RADIUS + 1) as u32;
    let (width, height) = (canvas.width, canvas.height);
    let mirror = |(x, y): (i32, i32)| (width - 1 - x, height - 1 - y);
    let mut passed_over = vec![false; canvas.cells.len()];
    let mut tier = Tier::Strict;
    let mut attempts = 0;
    while centres.len() < players {
        let no_site = MapScaleError::NoStartSite {
            start: centres.len(),
            players,
        };
        // The terrain as it is now: sites made so far count.
        let reached = canvas.reachable(first);
        let water = Counts::of(canvas, |index| canvas.cells[index].water);
        let ore = Counts::of(canvas, |index| canvas.cells[index].ore > 0);
        let open = Counts::of(canvas, |index| canvas.cells[index].passable);
        let bridges = Counts::of(canvas, |index| {
            canvas.looks[index] >> 4 == LOOK_FEATURE_BRIDGE
        });
        let qualifies = |(x, y): (i32, i32)| {
            let Some(index) = canvas.index(x, y) else {
                return false;
            };
            if x < EDGE_MARGIN
                || y < EDGE_MARGIN
                || x >= width - EDGE_MARGIN
                || y >= height - EDGE_MARGIN
                || !reached[index]
                || passed_over[index]
            {
                return false;
            }
            let wet = water.around(x, y, PAD_RADIUS);
            match tier {
                Tier::Strict => {
                    wet * 3 <= area
                        && open.around(x, y, PAD_RADIUS) * 2 >= area
                        && water.around(x, y, CORE_RADIUS) == 0
                        && ore.around(x, y, CORE_RADIUS) == 0
                        && bridges.around(x, y, PAD_RADIUS) == 0
                }
                Tier::Relaxed => wet * 2 <= area,
            }
        };
        let nearest = |cell: (i32, i32)| {
            centres
                .iter()
                .map(|&centre| distance_squared(cell, centre))
                .min()
                .unwrap_or(i64::MAX)
        };
        let cells = || (0..height).flat_map(|y| (0..width).map(move |x| (x, y)));
        // A site and its mirror, both as far from every start and from each other as possible.
        let pair = (players - centres.len() >= 2)
            .then(|| {
                cells()
                    .filter(|&cell| {
                        let other = mirror(cell);
                        (cell.1, cell.0) < (other.1, other.0) && qualifies(cell) && qualifies(other)
                    })
                    .map(|cell| {
                        let other = mirror(cell);
                        let room = nearest(cell)
                            .min(nearest(other))
                            .min(distance_squared(cell, other));
                        (room, cell)
                    })
                    .filter(|&(room, _)| room >= least)
                    // The first of the farthest: `max_by_key` alone would keep the last.
                    .max_by_key(|&(room, (x, y))| (room, std::cmp::Reverse((y, x))))
            })
            .flatten();
        let chosen = pair
            .map(|(_, cell)| (cell, Some(mirror(cell))))
            .or_else(|| {
                cells()
                    .filter(|&cell| qualifies(cell))
                    .map(|cell| (nearest(cell), cell))
                    .filter(|&(room, _)| room >= least)
                    .max_by_key(|&(room, (x, y))| (room, std::cmp::Reverse((y, x))))
                    .map(|(_, cell)| (cell, None))
            });
        let Some((cell, other)) = chosen else {
            // Nothing qualifies: fill in water if that has not been tried, else give up.
            if tier == Tier::Strict {
                tier = Tier::Relaxed;
                attempts = 0;
                passed_over.fill(false);
                continue;
            }
            return Err(no_site);
        };
        let mut others = centres.clone();
        others.extend(other);
        let planned = plan_site(canvas, cell, &others, false).and_then(|site| {
            let Some(other) = other else {
                return Some((site, None));
            };
            let mut others = centres.clone();
            others.push(cell);
            plan_site(canvas, other, &others, true).map(|mirrored| (site, Some(mirrored)))
        });
        let Some((site, mirrored)) = planned else {
            for cell in std::iter::once(cell).chain(other) {
                if let Some(index) = canvas.index(cell.0, cell.1) {
                    passed_over[index] = true;
                }
            }
            attempts += 1;
            if attempts >= MAX_SITE_ATTEMPTS {
                if tier == Tier::Strict {
                    tier = Tier::Relaxed;
                    attempts = 0;
                    passed_over.fill(false);
                    continue;
                }
                return Err(no_site);
            }
            continue;
        };
        for site in std::iter::once(&site).chain(&mirrored) {
            make_site(canvas, site, amount);
            centres.push(site.centre);
        }
    }
    Ok(())
}

/// The map of a match with `players` players on the map file whose terrain is `grid`, whose
/// looks are `looks` (row-major; cells beyond its end have look 0) and whose start positions are
/// `starts` (see the module documentation).
///
/// With two players, on a map with start positions for both, the result is the map file's own:
/// `grid`, `looks` and `starts` unchanged, cell for cell. Otherwise the terrain is enlarged to
/// [`enlarged_size`], the start positions move with it, and missing start positions are made, so
/// that the result has at least `players` of them (all of the file's, also when the match uses
/// fewer). Units and buildings on `grid` are not carried over: a map has none before match setup.
pub fn enlarged_map(
    grid: &Grid,
    looks: &[u8],
    starts: &[CellPos],
    players: usize,
) -> Result<EnlargedMap, MapScaleError> {
    let size = enlarged_size(grid.width(), grid.height(), players);
    if size == (grid.width(), grid.height()) && players <= starts.len() {
        return Ok(EnlargedMap {
            grid: grid.clone(),
            looks: looks.to_vec(),
            starts: starts.to_vec(),
        });
    }
    let columns = axis_map(grid.width(), size.0);
    let rows = axis_map(grid.height(), size.1);
    let blocked = Cell {
        passable: false,
        water: false,
        buildable: false,
        ore: 0,
    };
    let mut cells = Vec::with_capacity(columns.len() * rows.len());
    let mut shown = Vec::with_capacity(columns.len() * rows.len());
    // The cells shown from a cell of gems (RA5).
    let mut gems = Vec::new();
    for &row in &rows {
        for &column in &columns {
            let from = CellPos::new(
                i32::try_from(column).unwrap_or(i32::MAX),
                i32::try_from(row).unwrap_or(i32::MAX),
            );
            if grid.is_gems(from) {
                gems.push(cells.len());
            }
            cells.push(grid.cell(from).copied().unwrap_or(blocked));
            let index = u64::from(row) * u64::from(grid.width()) + u64::from(column);
            shown.push(
                usize::try_from(index)
                    .ok()
                    .and_then(|index| looks.get(index))
                    .copied()
                    .unwrap_or(0),
            );
        }
    }
    let too_large = MapScaleError::TooLarge {
        width: size.0,
        height: size.1,
    };
    let mut canvas = Canvas {
        width: i32::try_from(size.0).map_err(|_| too_large)?,
        height: i32::try_from(size.1).map_err(|_| too_large)?,
        cells,
        looks: shown,
    };
    let mut centres: Vec<(i32, i32)> = starts
        .iter()
        .map(|start| {
            (
                moved(
                    &columns,
                    grid.width(),
                    start.x.saturating_add(YARD_CENTRE_OFFSET),
                ),
                moved(
                    &rows,
                    grid.height(),
                    start.y.saturating_add(YARD_CENTRE_OFFSET),
                ),
            )
        })
        .collect();
    let before = (canvas.cells.clone(), canvas.looks.clone());
    make_starts(&mut canvas, &mut centres, players)?;
    let width = canvas.width;
    // Gems stay gems where a made start position changed nothing; ground it cleared or planted
    // with ore holds none.
    gems.retain(|&index| {
        canvas.cells[index] == before.0[index] && canvas.looks[index] == before.1[index]
    });
    // Discrete objects retain source-cell identity: duplicating terrain bands must not
    // duplicate one building, or merge two adjacent buildings of the same type.
    let anchors: Vec<_> = grid
        .world_features()
        .filter(|(_, feature)| *feature >= 11)
        .map(|(cell, feature)| {
            (
                CellPos::new(
                    moved(&columns, grid.width(), cell.x),
                    moved(&rows, grid.height(), cell.y),
                ),
                feature,
            )
        })
        .collect();
    let anchors: std::collections::BTreeMap<_, _> = anchors
        .into_iter()
        .filter(|(cell, feature)| {
            let index = cell.y as usize * width as usize + cell.x as usize;
            canvas
                .looks
                .get(index)
                .is_some_and(|look| look >> 4 == *feature)
                && before.0.get(index) == canvas.cells.get(index)
        })
        .collect();
    // A source anchor remains one occupied cell. The repeated terrain bands around it become
    // its clear apron, not invisible indestructible obstacles or extra buildings.
    for (index, look) in canvas.looks.iter_mut().enumerate() {
        if *look >> 4 >= 11 {
            let cell = CellPos::new(index as i32 % width, index as i32 / width);
            if !anchors.contains_key(&cell) {
                *look &= 15;
                let water = *look == 6;
                canvas.cells[index] = Cell {
                    passable: !water,
                    water,
                    buildable: !water,
                    ore: 0,
                };
            }
        }
    }
    let mut grid = Grid::from_cells(size.0, size.1, canvas.cells).ok_or(too_large)?;
    grid.mark_world_features(&canvas.looks);

    for index in gems {
        let index = i32::try_from(index).unwrap_or(i32::MAX);
        grid.set_gems(CellPos::new(index % width, index / width), true);
    }
    Ok(EnlargedMap {
        grid,
        looks: canvas.looks,
        starts: centres
            .into_iter()
            .map(|(x, y)| CellPos::new(x - YARD_CENTRE_OFFSET, y - YARD_CENTRE_OFFSET))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPEN: Cell = Cell {
        passable: true,
        water: false,
        buildable: true,
        ore: 0,
    };
    const ROCK: Cell = Cell {
        passable: false,
        water: false,
        buildable: false,
        ore: 0,
    };
    const WATER: Cell = Cell {
        passable: false,
        water: true,
        buildable: false,
        ore: 0,
    };
    const ORE: Cell = Cell {
        passable: true,
        water: false,
        buildable: false,
        ore: 300,
    };

    /// Looks as the game's maps use them: grass, water, a forest on grass, ore on grass.
    const GRASS: u8 = 1;
    const SEA: u8 = LOOK_GROUND_WATER;
    const FOREST: u8 = GRASS | 4 << 4;
    const ORE_LOOK: u8 = GRASS | LOOK_FEATURE_ORE << 4;

    fn look_of(cell: &Cell) -> u8 {
        match cell {
            cell if cell.water => SEA,
            cell if cell.ore > 0 => ORE_LOOK,
            cell if !cell.passable => FOREST,
            _ => GRASS,
        }
    }

    /// A map of `width` × `height` cells with the terrain `terrain` gives each cell, and looks to
    /// match.
    fn map(width: u32, height: u32, terrain: impl Fn(i32, i32) -> Cell) -> (Grid, Vec<u8>) {
        let cells: Vec<Cell> = (0..height as i32)
            .flat_map(|y| (0..width as i32).map(move |x| (x, y)))
            .map(|(x, y)| terrain(x, y))
            .collect();
        let looks = cells.iter().map(look_of).collect();
        (
            Grid::from_cells(width, height, cells).expect("a map"),
            looks,
        )
    }

    /// A fixed scramble of a cell's coordinates, for terrain without any regularity.
    fn noise(x: i32, y: i32) -> u32 {
        let mut n = (x as u32).wrapping_mul(0x85eb_ca6b) ^ (y as u32).wrapping_mul(0xc2b2_ae35);
        n ^= n >> 15;
        n = n.wrapping_mul(0x2c1b_3c6d);
        n ^ (n >> 13)
    }

    /// Scattered rock, ponds and ore, the same at a cell and at its point mirror.
    fn scattered(width: i32, height: i32) -> impl Fn(i32, i32) -> Cell {
        move |x, y| {
            let (mx, my) = (width - 1 - x, height - 1 - y);
            let (x, y) = if (y, x) <= (my, mx) { (x, y) } else { (mx, my) };
            match noise(x / 2, y / 2) % 10 {
                0 | 1 => ROCK,
                2 => WATER,
                3 => ORE,
                _ => OPEN,
            }
        }
    }

    fn all_cells(grid: &Grid) -> impl Iterator<Item = CellPos> + use<> {
        let (width, height) = (grid.width() as i32, grid.height() as i32);
        (0..height).flat_map(move |y| (0..width).map(move |x| CellPos::new(x, y)))
    }

    /// How many separate pieces the cells that `open` admits form (four-neighbour steps).
    fn pieces(grid: &Grid, open: impl Fn(&Cell) -> bool) -> usize {
        let width = grid.width() as i32;
        let admits = |cell: CellPos| grid.cell(cell).is_some_and(&open);
        let mut seen = vec![false; (grid.width() * grid.height()) as usize];
        let mut pieces = 0;
        for from in all_cells(grid) {
            if !admits(from)
                || std::mem::replace(&mut seen[(from.y * width + from.x) as usize], true)
            {
                continue;
            }
            pieces += 1;
            let mut queue = VecDeque::from([from]);
            while let Some(cell) = queue.pop_front() {
                for (dx, dy) in NEIGHBOURS {
                    let next = CellPos::new(cell.x + dx, cell.y + dy);
                    if admits(next)
                        && !std::mem::replace(&mut seen[(next.y * width + next.x) as usize], true)
                    {
                        queue.push_back(next);
                    }
                }
            }
        }
        pieces
    }

    fn symmetric(map: &EnlargedMap) -> bool {
        let (width, height) = (map.grid.width() as i32, map.grid.height() as i32);
        all_cells(&map.grid).all(|cell| {
            let mirror = CellPos::new(width - 1 - cell.x, height - 1 - cell.y);
            map.grid.cell(cell) == map.grid.cell(mirror)
                && map.looks[(cell.y * width + cell.x) as usize]
                    == map.looks[(mirror.y * width + mirror.x) as usize]
        })
    }

    fn centre(start: CellPos) -> (i32, i32) {
        (start.x + YARD_CENTRE_OFFSET, start.y + YARD_CENTRE_OFFSET)
    }

    #[test]
    fn the_size_grows_by_an_eighth_per_player_beyond_the_second_up_to_the_largest_map() {
        let sizes = |width, height| -> Vec<(u32, u32)> {
            (2..=MAX_MATCH_PLAYERS)
                .map(|players| enlarged_size(width, height, players))
                .collect()
        };
        let square = |sides: [u32; 9]| sides.map(|side| (side, side)).to_vec();
        // Sides that are multiples of 16 grow by exactly an eighth per player.
        assert_eq!(
            sizes(128, 128),
            square([128, 144, 160, 176, 192, 208, 224, 240, 256])
        );
        assert_eq!(
            sizes(64, 64),
            square([64, 72, 80, 88, 96, 104, 112, 120, 128])
        );
        assert_eq!(
            sizes(128, 96),
            [
                (128, 96),
                (144, 108),
                (160, 120),
                (176, 132),
                (192, 144),
                (208, 156),
                (224, 168),
                (240, 180),
                (256, 192)
            ]
        );
        // Others round the added cells up to an even number: 56 · 9/8 = 63 becomes 64.
        assert_eq!(
            sizes(56, 56),
            square([56, 64, 70, 78, 84, 92, 98, 106, 112])
        );
        assert_eq!(
            sizes(120, 120),
            square([120, 136, 150, 166, 180, 196, 210, 226, 240])
        );
        assert_eq!(sizes(7, 7)[8], (15, 15), "odd sides stay odd");
        // Fewer than two players and more than the most count as two and as the most.
        assert_eq!(enlarged_size(64, 48, 0), (64, 48));
        assert_eq!(enlarged_size(64, 48, 1), (64, 48));
        assert_eq!(enlarged_size(64, 48, 99), (128, 96));
        // No side exceeds the largest map, and the map keeps its proportions: both sides stop
        // at the last step that fits.
        assert_eq!(enlarged_size(MAX_MAP_SIDE, MAX_MAP_SIDE, 10), (256, 256));
        assert_eq!(enlarged_size(200, 100, 4), (250, 126));
        assert_eq!(enlarged_size(200, 100, 10), (250, 126));
        assert_eq!(enlarged_size(100, 200, 10), (126, 250));
        assert_eq!(enlarged_size(255, 16, 10), (255, 16));
        for width in 1..=MAX_MAP_SIDE {
            for players in 2..=MAX_MATCH_PLAYERS {
                let (wide, high) = enlarged_size(width, 96, players);
                assert!(wide <= MAX_MAP_SIDE && high <= MAX_MAP_SIDE);
                assert!(wide >= width && high >= 96);
                assert_eq!((wide - width) % 2, 0);
            }
        }
    }

    #[test]
    fn every_column_of_the_file_is_shown_in_order_and_mirrored_columns_stay_mirrored() {
        for source in 1..=96_u32 {
            for players in 2..=MAX_MATCH_PLAYERS {
                let (target, _) = enlarged_size(source, source, players);
                let map = axis_map(source, target);
                assert_eq!(map.len(), target as usize);
                assert_eq!(map[0], 0, "{source} -> {target}");
                assert_eq!(map[map.len() - 1], source - 1, "{source} -> {target}");
                for pair in map.windows(2) {
                    assert!(
                        pair[1] == pair[0] || pair[1] == pair[0] + 1,
                        "{source} -> {target}: {map:?}"
                    );
                }
                for (at, &shown) in map.iter().enumerate() {
                    assert_eq!(
                        map[map.len() - 1 - at],
                        source - 1 - shown,
                        "{source} -> {target}: {map:?}"
                    );
                }
                // A column of the file moves to a column that shows it, mirrored with it.
                for column in 0..source as i32 {
                    let at = moved(&map, source, column);
                    assert_eq!(map[at as usize], column as u32, "{source} -> {target}");
                    assert_eq!(
                        moved(&map, source, source as i32 - 1 - column),
                        target as i32 - 1 - at,
                        "{source} -> {target}"
                    );
                }
            }
        }
        // The same size: every column shows itself.
        assert_eq!(axis_map(5, 5), [0, 1, 2, 3, 4]);
        assert_eq!(axis_map(4, 8), [0, 0, 1, 1, 2, 2, 3, 3]);
    }

    #[test]
    fn two_players_play_the_map_file_itself() {
        let (grid, looks) = map(40, 32, scattered(40, 32));
        let starts = [
            CellPos::new(3, 3),
            CellPos::new(34, 26),
            CellPos::new(20, 5),
        ];
        let same = enlarged_map(&grid, &looks, &starts, 2).expect("two players");
        assert_eq!(same.grid, grid);
        assert_eq!(same.looks, looks);
        assert_eq!(same.starts, starts);
        // Also with fewer looks than cells (a map without any), which stay as they are.
        let bare = enlarged_map(&grid, &[], &starts, 2).expect("two players");
        assert_eq!((bare.grid, bare.looks), (grid, Vec::new()));
    }

    #[test]
    fn enlarged_terrain_keeps_every_piece_of_land_and_water_and_its_symmetry() {
        let (width, height) = (40, 32);
        let (grid, looks) = map(width, height, scattered(width as i32, height as i32));
        let starts: Vec<CellPos> = (0..MAX_MATCH_PLAYERS as i32)
            .map(|k| CellPos::new(2 + 3 * k, 2))
            .collect();
        let land = pieces(&grid, |cell| cell.passable);
        let sea = pieces(&grid, |cell| cell.water);
        assert!(
            land > 1 && sea > 1,
            "the terrain is in pieces: {land}, {sea}"
        );
        for players in 3..=MAX_MATCH_PLAYERS {
            // The file has a start for every player, so nothing but the size changes.
            let enlarged = enlarged_map(&grid, &looks, &starts, players).expect("enlarged");
            let size = enlarged_size(width, height, players);
            assert_eq!((enlarged.grid.width(), enlarged.grid.height()), size);
            assert_eq!(pieces(&enlarged.grid, |cell| cell.passable), land);
            assert_eq!(pieces(&enlarged.grid, |cell| cell.water), sea);
            assert!(symmetric(&enlarged), "{players} players");
            // Every cell shows a cell of the file, with its look.
            let (columns, rows) = (axis_map(width, size.0), axis_map(height, size.1));
            for cell in all_cells(&enlarged.grid) {
                let from = CellPos::new(
                    columns[cell.x as usize] as i32,
                    rows[cell.y as usize] as i32,
                );
                assert_eq!(enlarged.grid.cell(cell), grid.cell(from));
                assert_eq!(
                    enlarged.looks[(cell.y * size.0 as i32 + cell.x) as usize],
                    looks[(from.y * width as i32 + from.x) as usize]
                );
            }
            // Ore is repeated with its cells: more players, more ore.
            let ore = |grid: &Grid| -> u64 {
                all_cells(grid)
                    .map(|cell| u64::from(grid.cell(cell).map_or(0, |cell| cell.ore)))
                    .sum()
            };
            assert!(ore(&enlarged.grid) > ore(&grid));
            // The starts move with the ground their yards' centres stand on.
            for (start, moved) in starts.iter().zip(&enlarged.starts) {
                let (x, y) = centre(*moved);
                assert_eq!(
                    (columns[x as usize] as i32, rows[y as usize] as i32),
                    centre(*start)
                );
            }
        }
    }

    #[test]
    fn missing_starts_are_made_far_apart_in_mirrored_pairs_with_ground_and_ore() {
        // An open plain with a forest belt, a pond and one ore field per authored start.
        let (width, height) = (64, 64);
        let terrain = |x: i32, y: i32| {
            let (mx, my) = (width - 1 - x, height - 1 - y);
            let (x, y) = if (y, x) <= (my, mx) { (x, y) } else { (mx, my) };
            if (x - 14).pow(2) + (y - 6).pow(2) <= 9 {
                ORE
            } else if (28..32).contains(&x) && y < 24 {
                ROCK
            } else if (x - 44).pow(2) + (y - 20).pow(2) <= 30 {
                WATER
            } else {
                OPEN
            }
        };
        let (grid, looks) = map(width as u32, height as u32, terrain);
        let starts = [CellPos::new(5, 5), CellPos::new(56, 56)];
        for players in 3..=MAX_MATCH_PLAYERS {
            let enlarged = enlarged_map(&grid, &looks, &starts, players).expect("enlarged");
            // The same call gives the same map.
            assert_eq!(
                enlarged_map(&grid, &looks, &starts, players).as_ref(),
                Ok(&enlarged)
            );
            let (wide, high) = (enlarged.grid.width() as i32, enlarged.grid.height() as i32);
            assert_eq!(enlarged.starts.len(), players);
            let centres: Vec<(i32, i32)> = enlarged.starts.iter().map(|&s| centre(s)).collect();
            let least = i64::from(MIN_START_DISTANCE).pow(2);
            let land = Canvas {
                width: wide,
                height: high,
                cells: all_cells(&enlarged.grid)
                    .map(|cell| *enlarged.grid.cell(cell).expect("cell"))
                    .collect(),
                looks: enlarged.looks.clone(),
            };
            let reached = land.reachable((enlarged.starts[0].x, enlarged.starts[0].y));
            for (k, &(x, y)) in centres.iter().enumerate().skip(starts.len()) {
                for (other, &there) in centres.iter().enumerate() {
                    assert!(
                        other == k || distance_squared((x, y), there) >= least,
                        "{players} players: starts {k} and {other}"
                    );
                }
                assert!((EDGE_MARGIN..wide - EDGE_MARGIN).contains(&x));
                assert!((EDGE_MARGIN..high - EDGE_MARGIN).contains(&y));
                // The yard and the ring around it: open building ground, reachable, plain look.
                for dy in -CORE_RADIUS..=CORE_RADIUS {
                    for dx in -CORE_RADIUS..=CORE_RADIUS {
                        let index = land.index(x + dx, y + dy).expect("on the map");
                        assert_eq!(land.cells[index], OPEN);
                        assert_eq!(land.looks[index], GRASS);
                        assert!(reached[index]);
                    }
                }
                // Its own ore: within reach of the yard, off its apron, joined to the land.
                let reach = ORE_DISTANCE + ORE_RADIUS + 2;
                let mut ore = 0;
                for dy in -reach..=reach {
                    for dx in -reach..=reach {
                        let Some(index) = land.index(x + dx, y + dy) else {
                            continue;
                        };
                        if land.cells[index].ore == 0 {
                            continue;
                        }
                        assert!(
                            dx.abs() > APRON_RADIUS || dy.abs() > APRON_RADIUS,
                            "ore on the apron"
                        );
                        assert_eq!(land.looks[index], ORE_LOOK);
                        // As much ore per cell as the map's own fields.
                        assert_eq!(land.cells[index].ore, ORE.ore);
                        ore += usize::from(reached[index]);
                    }
                }
                assert!(ore >= MIN_ORE_CELLS, "{players} players: start {k}: {ore}");
            }
            // An even number of made starts on a symmetric map: mirrored pairs, a symmetric map.
            if (players - starts.len()).is_multiple_of(2) {
                assert!(symmetric(&enlarged), "{players} players");
                for pair in enlarged.starts.chunks(2) {
                    assert_eq!(
                        (pair[0].x + pair[1].x, pair[0].y + pair[1].y),
                        (wide - 3, high - 3),
                        "{players} players"
                    );
                }
            }
            // Nothing that was passable is blocked, and no water is filled in.
            let (columns, rows) = (
                axis_map(width as u32, wide as u32),
                axis_map(height as u32, high as u32),
            );
            for cell in all_cells(&enlarged.grid) {
                let from = CellPos::new(
                    columns[cell.x as usize] as i32,
                    rows[cell.y as usize] as i32,
                );
                let (before, after) = (grid.cell(from).expect("cell"), land.cell(cell.x, cell.y));
                let after = after.expect("cell");
                assert!(after.passable || !before.passable, "{cell:?}");
                assert_eq!(after.water, before.water, "{cell:?}");
            }
        }
    }

    #[test]
    fn a_made_start_in_a_forest_is_cleared_ground_that_keeps_the_maps_looks() {
        // Forest all over but for two clearings and a few lanes: no site is half open, so the
        // made starts are cut out of the forest.
        let (width, height) = (72, 72);
        let terrain = |x: i32, y: i32| {
            let near = |cx: i32, cy: i32| (x - cx).abs() <= 6 && (y - cy).abs() <= 6;
            if near(8, 8) || near(63, 63) || (x - y).abs() <= 1 || x % 12 == 0 {
                OPEN
            } else {
                ROCK
            }
        };
        let (grid, mut looks) = map(width, height, terrain);
        // Look 0 on the left half: a map without looks keeps none on made ground and ore.
        for y in 0..height as usize {
            for x in 0..width as usize / 2 {
                looks[y * width as usize + x] = 0;
            }
        }
        let starts = [CellPos::new(7, 7), CellPos::new(62, 62)];
        let enlarged = enlarged_map(&grid, &looks, &starts, 4).expect("enlarged");
        let wide = enlarged.grid.width() as i32;
        let look = |x: i32, y: i32| enlarged.looks[(y * wide + x) as usize];
        assert_eq!(enlarged.starts.len(), 4);
        for &start in &enlarged.starts[2..] {
            let (x, y) = centre(start);
            let mut cleared = 0;
            for (dx, dy) in pad_offsets() {
                let cell = *enlarged
                    .grid
                    .cell(CellPos::new(x + dx, y + dy))
                    .expect("cell");
                // The whole pad is open ground or ore: the forest is gone, its ground stays.
                assert!(cell.passable, "({dx}, {dy})");
                let auto = x + dx < wide / 2;
                if cell.ore == 0 {
                    assert!(cell.buildable);
                    cleared += 1;
                    assert_eq!(look(x + dx, y + dy), if auto { 0 } else { GRASS });
                } else {
                    assert_eq!(look(x + dx, y + dy), if auto { 0 } else { ORE_LOOK });
                    // A map without ore of its own gets the default amount.
                    assert_eq!(cell.ore, DEFAULT_ORE);
                }
            }
            assert!(cleared > 100, "{cleared}");
        }
    }

    #[test]
    fn a_pad_keeps_water_ore_and_bridges_beside_the_yard_and_clears_only_what_joins_it() {
        // A plain with, around the site at (20, 20): a pond and old ore on the pad's rim, a
        // bridge deck, a creek that cuts a corner of the pad off, and rock everywhere else on it.
        let bridge = LOOK_GROUND_WATER | LOOK_FEATURE_BRIDGE << 4;
        let site = (20, 20);
        let mut canvas = Canvas {
            width: 48,
            height: 48,
            cells: vec![OPEN; 48 * 48],
            looks: vec![GRASS; 48 * 48],
        };
        let mut set = |x: i32, y: i32, cell: Cell, look: u8| {
            let index = (y * 48 + x) as usize;
            canvas.cells[index] = cell;
            canvas.looks[index] = look;
        };
        for (dx, dy) in pad_offsets() {
            set(site.0 + dx, site.1 + dy, ROCK, FOREST);
        }
        set(site.0 + 5, site.1, WATER, SEA);
        set(site.0, site.1 + 5, ORE, ORE_LOOK);
        let deck = Cell { ore: 0, ..ORE };
        set(site.0 - 5, site.1, deck, bridge);
        // Water under the yard itself.
        set(site.0 + 1, site.1 + 1, WATER, SEA);
        // A creek across the pad's upper rows: what lies beyond cannot be walked to.
        for dx in -PAD_RADIUS..=PAD_RADIUS {
            set(site.0 + dx, site.1 - 5, WATER, SEA);
        }
        let before = (canvas.cells.clone(), canvas.looks.clone());
        let plan = plan_site(&canvas, site, &[], false).expect("room for ore");
        make_site(&mut canvas, &plan, 700);
        let at = |dx: i32, dy: i32| {
            let index = ((site.1 + dy) * 48 + site.0 + dx) as usize;
            (canvas.cells[index], canvas.looks[index])
        };
        // The yard's ground is all building ground: the water under it is filled in with the
        // ground of the pad's dry cells.
        for dy in -CORE_RADIUS..=CORE_RADIUS {
            for dx in -CORE_RADIUS..=CORE_RADIUS {
                assert_eq!(at(dx, dy), (OPEN, GRASS), "({dx}, {dy})");
            }
        }
        // Beside it, the pond, the old ore (with its own amount) and the bridge stay.
        assert_eq!(at(5, 0), (WATER, SEA));
        assert_eq!(at(0, 5), (ORE, ORE_LOOK));
        assert_eq!(at(-5, 0), (deck, bridge));
        // The creek stays, and so does the forest beyond it.
        assert_eq!(at(0, -5), (WATER, SEA));
        assert_eq!(at(0, -6), (ROCK, FOREST));
        assert_eq!(at(1, -6), (ROCK, FOREST));
        // The rest of the pad is cleared.
        assert_eq!(at(-4, 4), (OPEN, GRASS));
        assert_eq!(at(6, 2), (OPEN, GRASS));
        // New ore: 700 a cell, on the same ground, off the apron, at least the fewest cells.
        let planted: Vec<usize> = (0..canvas.cells.len())
            .filter(|&index| canvas.cells[index].ore == 700)
            .collect();
        assert!(planted.len() >= MIN_ORE_CELLS, "{}", planted.len());
        for index in planted {
            let (x, y) = ((index % 48) as i32, (index / 48) as i32);
            assert!((x - site.0).abs() > APRON_RADIUS || (y - site.1).abs() > APRON_RADIUS);
            assert_eq!(canvas.looks[index], ORE_LOOK);
            assert!(canvas.cells[index].passable && !canvas.cells[index].buildable);
        }
        // Nothing that was passable is blocked.
        for (cell, was) in canvas.cells.iter().zip(&before.0) {
            assert!(cell.passable || !was.passable);
        }
        // Another start nearby keeps its apron free of this site's ore.
        let mut canvas = Canvas {
            width: 48,
            height: 48,
            cells: before.0,
            looks: before.1,
        };
        let neighbours = [
            (site.0 + 12, site.1),
            (site.0, site.1 + 12),
            (site.0 - 12, site.1),
        ];
        let plan = plan_site(&canvas, site, &neighbours, false).expect("room for ore");
        make_site(&mut canvas, &plan, 700);
        for (index, cell) in canvas.cells.iter().enumerate() {
            let (x, y) = ((index % 48) as i32, (index / 48) as i32);
            let near = neighbours
                .iter()
                .any(|&(nx, ny)| (x - nx).abs() <= APRON_RADIUS && (y - ny).abs() <= APRON_RADIUS);
            assert!(cell.ore != 700 || !near, "({x}, {y})");
        }
    }

    #[test]
    fn made_starts_stay_on_dry_land_where_there_is_room() {
        // Two islands joined by a causeway in a sea: the made starts stand on the islands, and
        // the sea is the sea of the map file.
        let (width, height) = (96, 96);
        let land = |x: i32, y: i32| {
            (x - 24).pow(2) + (y - 24).pow(2) <= 22 * 22
                || (x - 71).pow(2) + (y - 71).pow(2) <= 22 * 22
                || (x - y).abs() <= 3
        };
        let (grid, looks) = map(width, height, |x, y| if land(x, y) { OPEN } else { WATER });
        let starts = [CellPos::new(12, 12), CellPos::new(81, 81)];
        let water = |grid: &Grid| {
            all_cells(grid)
                .filter(|&cell| grid.cell(cell).is_some_and(|cell| cell.water))
                .count()
        };
        // The same enlargement without made starts: the file lists four.
        let listed = [
            starts[0],
            starts[1],
            CellPos::new(30, 30),
            CellPos::new(63, 63),
        ];
        let plain = enlarged_map(&grid, &looks, &listed, 4).expect("enlarged");
        let enlarged = enlarged_map(&grid, &looks, &starts, 4).expect("room for four");
        assert_eq!(enlarged.starts.len(), 4);
        assert_eq!(
            water(&enlarged.grid),
            water(&plain.grid),
            "no water is filled in"
        );
        assert_ne!(enlarged.grid, plain.grid, "ground and ore are made");
        for &start in &enlarged.starts[2..] {
            let (x, y) = centre(start);
            for dy in -CORE_RADIUS..=CORE_RADIUS {
                for dx in -CORE_RADIUS..=CORE_RADIUS {
                    assert_eq!(
                        plain.grid.cell(CellPos::new(x + dx, y + dy)),
                        Some(&OPEN),
                        "the yard stands on what was dry land"
                    );
                }
            }
        }
    }

    #[test]
    fn a_map_with_water_under_every_site_has_it_filled_in_and_one_without_room_is_an_error() {
        // A plain with a puddle every five cells: no yard fits between them, so the puddles
        // under the made yards are filled in; the others stay.
        let (grid, looks) = map(64, 64, |x, y| {
            if x % 5 == 2 && y % 5 == 2 {
                WATER
            } else {
                OPEN
            }
        });
        let starts = [CellPos::new(5, 5), CellPos::new(56, 56)];
        let enlarged = enlarged_map(&grid, &looks, &starts, 3).expect("a relaxed site");
        let (x, y) = centre(enlarged.starts[2]);
        let mut puddles = 0;
        for (dx, dy) in pad_offsets() {
            let cell = *enlarged
                .grid
                .cell(CellPos::new(x + dx, y + dy))
                .expect("cell");
            let core = dx.abs() <= CORE_RADIUS && dy.abs() <= CORE_RADIUS;
            assert!(!core || cell == OPEN, "({dx}, {dy}): {cell:?}");
            puddles += usize::from(cell.water);
        }
        assert!(puddles > 0, "puddles beside the yard stay");
        let wide = enlarged.grid.width() as i32;
        assert_eq!(enlarged.looks[(y * wide + x) as usize], GRASS);

        // Ten players on 16 x 16 cells (32 x 32 enlarged): no room, reported with the start
        // that found none.
        let (grid, looks) = map(16, 16, |_, _| OPEN);
        let starts = [CellPos::new(1, 1), CellPos::new(12, 12)];
        let error = enlarged_map(&grid, &looks, &starts, 10).expect_err("no room");
        assert!(
            matches!(error, MapScaleError::NoStartSite { start, players: 10 } if start >= 2),
            "{error:?}"
        );
        assert!(error.to_string().contains("no room for start position"));
        // All sea but the two bases: nothing is reachable and dry enough.
        let (grid, looks) = map(64, 64, |x, y| {
            if (x < 12 && y < 12) || (x > 51 && y > 51) || x == y {
                OPEN
            } else {
                WATER
            }
        });
        assert_eq!(
            enlarged_map(
                &grid,
                &looks,
                &[CellPos::new(4, 4), CellPos::new(57, 57)],
                3
            ),
            Err(MapScaleError::NoStartSite {
                start: 2,
                players: 3
            })
        );
    }
}
