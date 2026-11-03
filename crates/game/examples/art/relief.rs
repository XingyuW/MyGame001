//! Relief of the painted maps (docs/DECISIONS.md, "Maps with relief"): what the generator adds
//! to a recipe's terrain so that a map is no longer one flat sheet.
//!
//! * [`Painter::enrich`] adds scenery to the recipe's map: downtowns grow more towers, the open
//!   country gets copses, crags and hamlets, and cities and suburbs parked vehicles.
//! * [`relief`] gives every cell a height (the map property `elevation`, 0 to
//!   `sim::MAX_ELEVATION` steps) and a decoration (`decor`, `looks::decor`).
//!
//! Heights follow the terrain: water lies at 0, beaches rise from it, the land rolls in hills,
//! towns stand on level ground, and rock is mountain, the higher the farther from its foot. Bases
//! stand on level pads. Wherever units can walk from a cell to its neighbour (any land that is
//! not rock), the two differ by at most one step, so every slope is a ramp and the simulation's
//! terrain (where units go) is exactly the recipe's; only rock and the shore have cliffs.
//!
//! Everything is integer arithmetic on point-symmetric inputs, so a map's relief is
//! point-symmetric too, and the same on every machine.

use super::super::looks::{Feature, Ground, decode, decor};
use super::super::{ADOBE, BRIDGE, COTTAGE, PARKED_CAR, SNOW_COTTAGE, TILES, WATER};
use super::{
    BASALT, CITY, CROPS, DESERT, DESERT_CITY, DESERT_TOWER, DIRT, FARMHOUSE, FOREST, GRASS, ICE,
    ICEBERG, LAVA, MARSH, MESA, PALMS, PLAZA, Painter, ROCK, SAND, SNOW, SNOW_CITY, SNOW_PINES,
    SNOW_ROCK, SNOW_TOWER, STREET, TOWER, fbm, hash, is_urban,
};

/// The highest elevation, in steps (`sim::MAX_ELEVATION`).
pub const MAX_HEIGHT: i32 = 15;

/// Cells from a start's yard centre (Chebyshev) that lie level with it.
const PAD: i32 = 9;

/// How a map's land lies: its plain height, how high its hills roll and how wide they are,
/// and how many steps rock rises per cell from its foot.
#[derive(Clone, Copy)]
struct Theme {
    base: i32,
    hills: i32,
    scale: i32,
    mountain: i32,
}

fn theme(id: &str) -> Theme {
    let rugged = Theme {
        base: 3,
        hills: 6,
        scale: 44,
        mountain: 3,
    };
    let lowland = Theme {
        base: 2,
        hills: 2,
        scale: 56,
        mountain: 2,
    };
    match id {
        "highland_plateau" => Theme {
            base: 4,
            hills: 7,
            scale: 40,
            mountain: 3,
        },
        "river_gorge" | "red_canyon" | "badlands" | "crater_field" | "coastal_cliffs"
        | "glacier_pass" | "frostline" | "fjord_coast" | "twin_volcanoes" | "ember_caldera"
        | "continental_divide" | "wheel_of_worlds" | "tundra_steppe" => rugged,
        "polder_lowlands" | "canal_city" | "thousand_lakes" | "black_water" | "mangrove_coast"
        | "jungle_delta" | "pacific_atoll" | "coral_archipelago" | "nile_valley"
        | "steel_harbor" | "liberty_bay" | "arctic_floes" | "frozen_sound" | "ice_shelf_rift" => {
            lowland
        }
        "rice_terraces" => Theme {
            base: 2,
            hills: 8,
            scale: 36,
            mountain: 2,
        },
        _ => Theme {
            base: 2,
            hills: 4,
            scale: 48,
            mountain: 2,
        },
    }
}

/// What a cell is to the relief.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Open water (and a pier hut standing in it): sea level.
    Water,
    /// Ice over water and bridge decks: passable, just above the water.
    Deck,
    /// A lava flow: low, in its channel.
    Lava,
    /// Rock of any environment: mountain, or a flat-topped mesa.
    Rock,
    Mesa,
    /// Everything else: the ground units cross, and the woods, houses and towns on it.
    Land,
}

fn kind_of(tile: u32) -> Kind {
    match tile {
        WATER | ICEBERG => {
            if tile == ICEBERG {
                Kind::Rock
            } else {
                Kind::Water
            }
        }
        ICE | BRIDGE => Kind::Deck,
        LAVA => Kind::Lava,
        MESA => Kind::Mesa,
        ROCK | SNOW_ROCK | BASALT => Kind::Rock,
        _ => match decode(TILES[tile as usize].look) {
            // The pier repair hut stands in the water.
            Some((Ground::Water, _)) => Kind::Water,
            _ => Kind::Land,
        },
    }
}

const STEPS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// The relief of a finished map: the height and decoration of every cell, row-major.
pub struct Relief {
    pub elevation: Vec<u8>,
    pub decor: Vec<u8>,
}

/// Breadth-first distance (four-neighbour steps, capped at `cap`) from the cells where `seed`
/// holds, through the cells where `through` holds.
fn distances(
    w: i32,
    h: i32,
    cap: i32,
    seed: impl Fn(usize) -> bool,
    through: impl Fn(usize) -> bool,
) -> Vec<i32> {
    let mut d = vec![cap; (w * h) as usize];
    let mut queue = std::collections::VecDeque::new();
    for (i, distance) in d.iter_mut().enumerate() {
        if seed(i) {
            *distance = 0;
            queue.push_back(i);
        }
    }
    while let Some(i) = queue.pop_front() {
        let (x, y) = (i as i32 % w, i as i32 / w);
        let next = d[i] + 1;
        if next >= cap {
            continue;
        }
        for (dx, dy) in STEPS {
            let (nx, ny) = (x + dx, y + dy);
            if (0..w).contains(&nx) && (0..h).contains(&ny) {
                let j = (ny * w + nx) as usize;
                if d[j] > next && through(j) {
                    d[j] = next;
                    queue.push_back(j);
                }
            }
        }
    }
    d
}

/// The relief of the map `id` of `w` × `h` cells with the finished `tiles` and the start
/// positions `starts` (yard origins).
pub fn relief(id: &str, w: i32, h: i32, tiles: &[u32], starts: &[(i32, i32)]) -> Relief {
    let theme = theme(id);
    let n = (w * h) as usize;
    let at = |x: i32, y: i32| (y * w + x) as usize;
    let kinds: Vec<Kind> = tiles.iter().map(|&t| kind_of(t)).collect();
    // Point-symmetric noise, as the painter's.
    let noise = |x: i32, y: i32, scale: i32, seed: u32| -> i64 {
        let v = (fbm(x, y, scale, seed) + fbm(w - 1 - x, h - 1 - y, scale, seed)) / 2;
        (512 + (v - 512) * 8 / 5).clamp(0, 1024)
    };
    let seed = hash(
        w,
        h,
        id.len() as u32 * 7919 + id.bytes().map(u32::from).sum::<u32>(),
    );

    // Land: rolling hills, level towns, and a shore that rises one step a cell from the water.
    let water_distance = distances(
        w,
        h,
        MAX_HEIGHT + 1,
        |i| matches!(kinds[i], Kind::Water | Kind::Deck),
        |i| kinds[i] != Kind::Water,
    );
    let mut target = vec![0_i32; n];
    for y in 0..h {
        for x in 0..w {
            let i = at(x, y);
            let tile = tiles[i];
            let urban = is_urban(tile) || tile == PARKED_CAR;
            let (scale, amplitude) = if urban {
                // A town lies level, terraced only where the land falls far.
                (theme.scale * 4, theme.hills / 2)
            } else {
                (theme.scale, theme.hills)
            };
            let roll = noise(x, y, scale, seed) * i64::from(amplitude) / 1024;
            let mut height = theme.base + roll as i32;
            if tile == MARSH || tile == super::MANGROVE {
                height = height.min(1);
            }
            target[i] = height.min(water_distance[i]).clamp(1, MAX_HEIGHT - 2);
        }
    }
    // Level pads for the bases, and a cone around them that the slope limit below cannot cut
    // into: no pad loses a step to lower ground around it.
    let land = |i: usize| matches!(kinds[i], Kind::Land | Kind::Deck);
    let mut floor = vec![i32::MIN; n];
    for &(sx, sy) in starts {
        let (cx, cy) = (sx + 1, sy + 1);
        if !(0..w).contains(&cx) || !(0..h).contains(&cy) {
            continue;
        }
        let level = target[at(cx, cy)];
        let pad = distances(
            w,
            h,
            level + 1,
            |i| {
                let (x, y) = (i as i32 % w, i as i32 / w);
                (x - cx).abs().max((y - cy).abs()) <= PAD && land(i)
            },
            land,
        );
        for i in 0..n {
            if pad[i] <= level {
                floor[i] = floor[i].max(level - pad[i]);
                if pad[i] == 0 {
                    target[i] = level;
                }
            }
        }
    }
    for i in 0..n {
        if land(i) {
            target[i] = target[i].max(floor[i]);
        }
    }
    // Ice and bridges lie just above the water, which the shore then descends to.
    for i in 0..n {
        if kinds[i] == Kind::Deck {
            target[i] = if tiles[i] == BRIDGE { 1 } else { 0 };
        }
    }
    // The slope limit: on land, no cell lies more than a step above a neighbour. The lower
    // envelope, by increasing height from every cell (a Dijkstra search with unit steps).
    let mut height = target.clone();
    let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); (MAX_HEIGHT + 2) as usize];
    for i in 0..n {
        if land(i) {
            buckets[height[i].clamp(0, MAX_HEIGHT + 1) as usize].push(i);
        }
    }
    for level in 0..buckets.len() {
        while let Some(i) = buckets[level].pop() {
            if height[i] != level as i32 {
                continue;
            }
            let (x, y) = (i as i32 % w, i as i32 / w);
            for (dx, dy) in STEPS {
                let (nx, ny) = (x + dx, y + dy);
                if !(0..w).contains(&nx) || !(0..h).contains(&ny) {
                    continue;
                }
                let j = at(nx, ny);
                // Decks are as low as they are; the land comes down to them.
                if land(j) && kinds[j] != Kind::Deck && height[j] > level as i32 + 1 {
                    height[j] = level as i32 + 1;
                    buckets[level + 1].push(j);
                }
            }
        }
    }

    // Rock: mountains rise from their foot, the higher the deeper inside the rock; mesas are
    // flat-topped tables. Lava runs low in its channel.
    let rock = |i: usize| matches!(kinds[i], Kind::Rock | Kind::Mesa);
    let depth = distances(w, h, 12, |i| !rock(i), |_| true);
    for y in 0..h {
        for x in 0..w {
            let i = at(x, y);
            match kinds[i] {
                Kind::Water => height[i] = 0,
                Kind::Lava => height[i] = 1,
                Kind::Rock | Kind::Mesa => {
                    // The foot: the highest land within three cells.
                    let mut foot = if tiles[i] == ICEBERG { 0 } else { theme.base };
                    for yy in y - 3..=y + 3 {
                        for xx in x - 3..=x + 3 {
                            if (0..w).contains(&xx) && (0..h).contains(&yy) {
                                let j = at(xx, yy);
                                if land(j) {
                                    foot = foot.max(height[j]);
                                }
                            }
                        }
                    }
                    let d = depth[i];
                    let rise = if kinds[i] == Kind::Mesa {
                        4 + d.min(2)
                    } else {
                        let crag = (noise(x, y, 7, seed ^ 0x51) - 512) * 3 / 512;
                        1 + theme.mountain * d + crag as i32
                    };
                    height[i] = (foot + rise.max(1)).clamp(1, MAX_HEIGHT);
                }
                _ => {}
            }
        }
    }
    // Guarantee of symmetry: the second half takes its mirror's heights.
    for i in n / 2..n {
        height[i] = height[n - 1 - i];
    }
    let elevation: Vec<u8> = height
        .iter()
        .map(|&h| h.clamp(0, MAX_HEIGHT) as u8)
        .collect();
    let decor = decorate(w, h, tiles, &elevation, starts);
    Relief { elevation, decor }
}

/// The decoration of every cell (0: none), point-symmetric.
fn decorate(w: i32, h: i32, tiles: &[u32], elevation: &[u8], starts: &[(i32, i32)]) -> Vec<u8> {
    let n = (w * h) as usize;
    let at = |x: i32, y: i32| (y * w + x) as usize;
    let tile =
        |x: i32, y: i32| ((0..w).contains(&x) && (0..h).contains(&y)).then(|| tiles[at(x, y)]);
    let near_start = |x: i32, y: i32, r: i32| {
        starts
            .iter()
            .any(|&(sx, sy)| (x - (sx + 1)).abs() <= r && (y - (sy + 1)).abs() <= r)
    };
    let mut decor = vec![0_u8; n];
    for y in 0..h {
        for x in 0..w {
            let i = at(x, y);
            if i >= n.div_ceil(2) {
                continue;
            }
            let t = tiles[i];
            let roll = hash(x, y, 0xdec0) % 1000;
            let around = |want: &dyn Fn(u32) -> bool| {
                [(1, 0), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .filter(|&&(dx, dy)| tile(x + dx, y + dy).is_some_and(want))
                    .count()
            };
            let ground = decode(TILES[t as usize].look).map(|(g, _)| g);
            let feature = decode(TILES[t as usize].look).map(|(_, f)| f);
            let by_water = around(&|n| n == WATER) > 0;
            let code = match t {
                PARKED_CAR => decor::VEHICLE,
                TOWER | SNOW_TOWER | DESERT_TOWER => {
                    // Downtown: towers among towers become skyscrapers.
                    let towers = (-1..=1)
                        .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
                        .filter(|&(dx, dy)| {
                            tile(x + dx, y + dy)
                                .is_some_and(|n| [TOWER, SNOW_TOWER, DESERT_TOWER].contains(&n))
                        })
                        .count();
                    if towers >= 4 && roll < 450 {
                        decor::SKYSCRAPER
                    } else {
                        0
                    }
                }
                CITY | SNOW_CITY | DESERT_CITY => match roll {
                    0..=17 => decor::LANDMARK,
                    18..=35 => decor::URBAN_TALL,
                    _ => 0,
                },
                PLAZA if !near_start(x, y, 8) => {
                    if roll < 260 {
                        decor::URBAN
                    } else {
                        0
                    }
                }
                ROCK | SNOW_ROCK | MESA | BASALT => {
                    if elevation[i] >= 5 && roll < 700 {
                        decor::CRAG
                    } else {
                        0
                    }
                }
                FARMHOUSE => {
                    if roll < 300 {
                        decor::RURAL_TALL
                    } else {
                        0
                    }
                }
                ADOBE => {
                    if roll < 250 {
                        decor::ARID_TALL
                    } else {
                        0
                    }
                }
                SNOW_COTTAGE => {
                    if roll < 250 {
                        decor::COLD_TALL
                    } else {
                        0
                    }
                }
                COTTAGE if by_water => {
                    if roll < 350 {
                        decor::COAST_TALL
                    } else {
                        0
                    }
                }
                WATER => {
                    let shore = (-2..=2)
                        .flat_map(|dy| (-2..=2).map(move |dx| (dx, dy)))
                        .any(|(dx, dy)| tile(x + dx, y + dy).is_some_and(|n| n != WATER));
                    if shore && roll < 45 { decor::SEA } else { 0 }
                }
                _ if near_start(x, y, 7) => 0,
                SAND if by_water => {
                    if roll < 70 {
                        decor::BEACH
                    } else {
                        0
                    }
                }
                CROPS => {
                    if roll < 45 {
                        decor::RURAL
                    } else {
                        0
                    }
                }
                SNOW => {
                    if roll < 40 {
                        decor::COLD
                    } else {
                        0
                    }
                }
                DESERT | SAND => {
                    if roll < 40 {
                        decor::ARID
                    } else {
                        0
                    }
                }
                GRASS | DIRT => {
                    if roll < 70 {
                        decor::CLUTTER
                    } else {
                        0
                    }
                }
                _ => {
                    let _ = (ground, feature);
                    0
                }
            };
            decor[i] = code;
            decor[n - 1 - i] = code;
        }
    }
    decor
}

impl Painter {
    /// Scenery for the open country and the towns of a finished recipe (run before its neutral
    /// sites): downtowns grow towers, the open country copses, crags and hamlets, and towns and
    /// suburbs parked vehicles. Nothing is placed near a base, nothing on roads, water or ore,
    /// and copses and crags only where the country is open all around, so no way is walled off;
    /// passable cells this leaves unreachable become woods as in [`Painter::finish`].
    pub fn enrich(&mut self, seed: u32) {
        let (w, h) = (self.w, self.h);
        // Downtown: city blocks among towers rise.
        let mut grow = Vec::new();
        for y in 0..h / 2 {
            for x in 0..w {
                let t = self.get(x, y);
                let tower = self.biome(x, y).tower;
                if ![CITY, SNOW_CITY, DESERT_CITY].contains(&t) {
                    continue;
                }
                let towers = (-1..=1)
                    .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
                    .filter(|&(dx, dy)| {
                        self.inside(x + dx, y + dy)
                            && [TOWER, SNOW_TOWER, DESERT_TOWER].contains(&self.get(x + dx, y + dy))
                    })
                    .count();
                if towers >= 3 && hash(x, y, seed ^ 0x70e5) % 100 < 45 {
                    grow.push((x, y, tower));
                }
            }
        }
        for (x, y, t) in grow {
            self.set(x, y, t);
        }
        self.mirror();
        // The open country: something every 11 cells or so where it is open all around.
        let spacing = 11;
        for gy in 0..=h / 2 / spacing {
            for gx in 0..=w / spacing {
                let roll = hash(gx, gy, seed ^ 0xe41c);
                let x = gx * spacing + (roll % spacing as u32) as i32;
                let y = gy * spacing + ((roll >> 8) % spacing as u32) as i32;
                if y >= h / 2 - 3 || !self.inside(x, y) || self.near_start(x, y, 16) {
                    continue;
                }
                let open = (-4..=4).all(|dy| {
                    (-4..=4).all(|dx| {
                        self.inside(x + dx, y + dy) && self.is_plain(self.get(x + dx, y + dy))
                    })
                });
                if !open {
                    continue;
                }
                let biome = self.biome(x, y);
                let local = seed ^ (roll >> 12);
                match (roll >> 20) % 10 {
                    0..=4 => self.patch(x, y, 1 + (roll >> 24) as i32 % 2, 1, local, biome.forest),
                    5 | 6 => self.patch(x, y, 1, 1, local, biome.cliff),
                    7 => self.village(x, y, 2, local),
                    _ => {}
                }
            }
        }
        // Parked vehicles: on plazas beside a street, and on driveways beside country houses.
        let mut parked = Vec::new();
        for y in 0..h / 2 {
            for x in 0..w {
                if self.near_start(x, y, 12) {
                    continue;
                }
                let t = self.get(x, y);
                let beside = |want: &[u32]| {
                    STEPS.iter().any(|&(dx, dy)| {
                        self.inside(x + dx, y + dy) && want.contains(&self.get(x + dx, y + dy))
                    })
                };
                let roll = hash(x, y, seed ^ 0xca75) % 100;
                if t == PLAZA && beside(&[STREET]) && roll < 22 {
                    parked.push((x, y));
                }
                if t == GRASS
                    && beside(&[COTTAGE])
                    && beside(&[super::ROAD, super::TRACK])
                    && roll < 30
                {
                    parked.push((x, y));
                }
            }
        }
        for (x, y) in parked {
            self.set(x, y, PARKED_CAR);
        }
        self.mirror();
        self.fill_pockets();
        self.mirror();
        let _ = (FOREST, PALMS, SNOW_PINES, Feature::None);
    }
}
