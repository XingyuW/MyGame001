//! The large mixed-environment skirmish maps (docs/DECISIONS.md, "Large maps with environments").
//!
//! Each map is painted onto a canvas by a recipe of shapes (coasts, rivers, cities, forests,
//! ridges, roads), then finished by passes that every map shares: beaches, cleared base sites,
//! connectivity and point symmetry. Everything is integer arithmetic on a fixed seed, so the
//! output is the same on every machine and `tests/m7_maps.rs` can compare it byte for byte.
//!
//! Symmetry: every placing tool (blobs, ore, roads, villages) also paints the point-mirror of what
//! it places, region fills use point-symmetric conditions, and all noise is symmetrised. The
//! finished map then takes rows `H/2..H` from the mirror of rows `0..H/2` as a guarantee, so both
//! halves are equal and so are mirrored start positions.
//!
//! The recipes are written for maps of 192 to 256 cells a side (docs/DECISIONS.md, "Maps at the
//! engine's largest size"): coordinates are cells of the finished map. A recipe paints the lie of
//! the land, then [`Painter::landmarks`] scatters what belongs to each environment over the open
//! country, then come the roads, the ore ([`Painter::home_ore`] finds room for two fields beside
//! every base) and [`Painter::finish`]. The maps made for that size are in `frontier_maps.rs`.
use super::{
    ADOBE, ASH, ASH_HOUSE, ASH_ORE, ASH_ROAD, ASH_WOODS, BASALT, BRIDGE, CITY, COTTAGE, CROPS,
    DESERT, DESERT_CITY, DESERT_ORE, DESERT_ROAD, DESERT_TOWER, DIRT, FARMHOUSE, FOREST, GRASS,
    ICE, ICEBERG, LAVA, MANGROVE, MARSH, MARSH_ORE, MESA, ORCHARD, ORE, PALMS, PLAZA, ROAD, ROCK,
    SAND, SNOW, SNOW_CITY, SNOW_COTTAGE, SNOW_ORE, SNOW_PINES, SNOW_ROAD, SNOW_ROCK, SNOW_TOWER,
    STREET, TILES, TOWER, TRACK, WATER,
};

#[path = "frontier_maps.rs"]
mod frontier_maps;

/// Tiles that make up one environment.
#[derive(Clone, Copy)]
pub struct Biome {
    pub plain: u32,
    pub ore: u32,
    pub cliff: u32,
    pub forest: u32,
    pub house: u32,
    pub road: u32,
    /// Ground beside water: passable, not buildable (a beach), or `plain`.
    pub shore: u32,
    /// A city block and a tower of a town in this environment.
    pub block: u32,
    pub tower: u32,
}

pub const TEMPERATE: Biome = Biome {
    plain: GRASS,
    ore: ORE,
    cliff: ROCK,
    forest: FOREST,
    house: COTTAGE,
    road: ROAD,
    shore: SAND,
    block: CITY,
    tower: TOWER,
};
pub const ARCTIC: Biome = Biome {
    plain: SNOW,
    ore: SNOW_ORE,
    cliff: SNOW_ROCK,
    forest: SNOW_PINES,
    house: SNOW_COTTAGE,
    road: SNOW_ROAD,
    shore: SNOW,
    block: SNOW_CITY,
    tower: SNOW_TOWER,
};
pub const DESERT_BIOME: Biome = Biome {
    plain: DESERT,
    ore: DESERT_ORE,
    cliff: MESA,
    forest: PALMS,
    house: ADOBE,
    road: DESERT_ROAD,
    shore: SAND,
    block: DESERT_CITY,
    tower: DESERT_TOWER,
};
/// Tropical islands: grass and jungle inland, palms on sandy shores.
pub const TROPIC: Biome = Biome {
    plain: GRASS,
    ore: ORE,
    cliff: ROCK,
    forest: FOREST,
    house: COTTAGE,
    road: ROAD,
    shore: SAND,
    block: CITY,
    tower: TOWER,
};

/// Volcanic country: ash plains, basalt, lava (painted by the recipes) and black beaches.
pub const VOLCANIC: Biome = Biome {
    plain: ASH,
    ore: ASH_ORE,
    cliff: BASALT,
    forest: ASH_WOODS,
    house: ASH_HOUSE,
    road: ASH_ROAD,
    shore: ASH,
    block: CITY,
    tower: TOWER,
};

/// The tiles of a town, whatever its environment.
fn is_urban(tile: u32) -> bool {
    [
        PLAZA,
        STREET,
        CITY,
        TOWER,
        SNOW_CITY,
        SNOW_TOWER,
        DESERT_CITY,
        DESERT_TOWER,
    ]
    .contains(&tile)
}

/// Trees of any environment: ore fields and base sites clear them.
fn is_woods(tile: u32) -> bool {
    [FOREST, PALMS, SNOW_PINES, ASH_WOODS, ORCHARD, MANGROVE].contains(&tile)
}

/// Sixteen directions around a point, nine cells out: where a base's ore fields may lie.
const RING: [(i32, i32); 16] = [
    (9, 0),
    (8, 4),
    (6, 6),
    (4, 8),
    (0, 9),
    (-4, 8),
    (-6, 6),
    (-8, 4),
    (-9, 0),
    (-8, -4),
    (-6, -6),
    (-4, -8),
    (0, -9),
    (4, -8),
    (6, -6),
    (8, -4),
];

// ---------------------------------------------------------------------------------------------
// Integer noise.

pub fn hash(x: i32, y: i32, seed: u32) -> u32 {
    let mut n = (x as u32).wrapping_mul(0x85eb_ca6b)
        ^ (y as u32).wrapping_mul(0xc2b2_ae35)
        ^ seed.wrapping_mul(0x27d4_eb2f);
    n ^= n >> 16;
    n = n.wrapping_mul(0x7feb_352d);
    n ^= n >> 15;
    n = n.wrapping_mul(0x846c_a68b);
    n ^ (n >> 16)
}

/// Smooth value noise in `0..=1024`, with features about `scale` cells across.
fn value(x: i32, y: i32, scale: i32, seed: u32) -> i64 {
    let scale = scale.max(1);
    let (gx, gy) = (x.div_euclid(scale), y.div_euclid(scale));
    let (fx, fy) = (x.rem_euclid(scale), y.rem_euclid(scale));
    let corner = |dx: i32, dy: i32| i64::from(hash(gx + dx, gy + dy, seed) % 1025);
    let ease = |f: i32| {
        let t = i64::from(f) * 1024 / i64::from(scale);
        t * t * (3072 - 2 * t) / (1024 * 1024)
    };
    let (sx, sy) = (ease(fx), ease(fy));
    let top = corner(0, 0) * (1024 - sx) + corner(1, 0) * sx;
    let bottom = corner(0, 1) * (1024 - sx) + corner(1, 1) * sx;
    (top * (1024 - sy) + bottom * sy) / (1024 * 1024)
}

/// Three octaves of value noise, `0..=1024`.
fn fbm(x: i32, y: i32, scale: i32, seed: u32) -> i64 {
    (value(x, y, scale, seed) * 4
        + value(x, y, scale / 2, seed ^ 0x9e37) * 2
        + value(x, y, scale / 4, seed ^ 0x7f4a))
        / 7
}

// ---------------------------------------------------------------------------------------------
// The canvas and its painting tools.

pub struct Painter {
    pub w: i32,
    pub h: i32,
    pub tiles: Vec<u32>,
    pub starts: Vec<(i32, i32)>,
    /// Half the width of a city's streets in doubled cells: 1 gives streets two cells wide, 3
    /// boulevards four cells wide.
    pub avenue: i32,
    /// The environment at each cell, for tools that place "the local" tree, road or ore.
    biome: fn(&Painter, i32, i32) -> Biome,
}

impl Painter {
    pub fn new(
        w: u32,
        h: u32,
        starts: &[(u32, u32)],
        biome: fn(&Painter, i32, i32) -> Biome,
    ) -> Self {
        let mut painter = Painter {
            w: w as i32,
            h: h as i32,
            tiles: Vec::new(),
            starts: starts.iter().map(|&(x, y)| (x as i32, y as i32)).collect(),
            avenue: 1,
            biome,
        };
        painter.tiles = (0..painter.h)
            .flat_map(|y| (0..w as i32).map(move |x| (x, y)))
            .map(|(x, y)| (painter.biome)(&painter, x, y).plain)
            .collect();
        painter
    }

    pub fn biome(&self, x: i32, y: i32) -> Biome {
        (self.biome)(self, x, y)
    }

    pub fn inside(&self, x: i32, y: i32) -> bool {
        (0..self.w).contains(&x) && (0..self.h).contains(&y)
    }

    pub fn get(&self, x: i32, y: i32) -> u32 {
        self.tiles[(y * self.w + x) as usize]
    }

    pub fn set(&mut self, x: i32, y: i32, tile: u32) {
        if self.inside(x, y) {
            let index = (y * self.w + x) as usize;
            self.tiles[index] = tile;
        }
    }

    /// Point-symmetric noise, so that the two halves of the map meet without a seam.
    pub fn noise(&self, x: i32, y: i32, scale: i32, seed: u32) -> i64 {
        let n = (fbm(x, y, scale, seed) + fbm(self.w - 1 - x, self.h - 1 - y, scale, seed)) / 2;
        // Averaging two fields halves their contrast; restore most of it.
        (512 + (n - 512) * 8 / 5).clamp(0, 1024)
    }

    /// Point-symmetric noise as a displacement of up to `amp` cells either way.
    pub fn wob(&self, x: i32, y: i32, scale: i32, seed: u32, amp: i32) -> i32 {
        ((self.noise(x, y, scale, seed) - 512) * i64::from(amp) / 512) as i32
    }

    /// A displacement of up to about `amp` cells along a line of `n` cells, odd about its middle:
    /// `meander(n - 1 - t) == -meander(t)`. A river at `v == 2 * meander(x)` (centred
    /// coordinates) is therefore its own point mirror, and still winds.
    pub fn meander(&self, t: i32, n: i32, scale: i32, seed: u32, amp: i32) -> i32 {
        let d = fbm(t, 0, scale, seed) - fbm(n - 1 - t, 0, scale, seed);
        (d * i64::from(amp) / 384).clamp(-i64::from(amp), i64::from(amp)) as i32
    }

    /// Whether (x, y) lies within `r` cells (Chebyshev) of a start's yard.
    pub fn near_start(&self, x: i32, y: i32, r: i32) -> bool {
        self.starts
            .iter()
            .any(|&(sx, sy)| (x - (sx + 1)).abs() <= r + 1 && (y - (sy + 1)).abs() <= r + 1)
    }

    /// A thick stroke of `tile` through `points` (a disc of radius `r` along each segment), and
    /// its mirror.
    pub fn stroke(&mut self, points: &[(i32, i32)], r: i32, tile: u32) {
        self.stroke_with(points, r, |_, _, _| tile);
    }

    /// [`Painter::stroke`] with the tile chosen per cell.
    pub fn stroke_with(
        &mut self,
        points: &[(i32, i32)],
        r: i32,
        tile: impl Fn(&Painter, i32, i32) -> u32,
    ) {
        for pair in points.windows(2) {
            let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
            let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
            for k in 0..=steps {
                let x = x0 + (x1 - x0) * k / steps;
                let y = y0 + (y1 - y0) * k / steps;
                let (mx, my) = self.mirror_of(x, y);
                for (cx, cy) in [(x, y), (mx, my)] {
                    for yy in cy - r..=cy + r {
                        for xx in cx - r..=cx + r {
                            if self.inside(xx, yy)
                                && (xx - cx).pow(2) + (yy - cy).pow(2) <= r * r + r
                            {
                                let t = tile(self, xx, yy);
                                self.set(xx, yy, t);
                            }
                        }
                    }
                }
            }
        }
    }

    /// The point-mirror of a cell.
    pub fn mirror_of(&self, x: i32, y: i32) -> (i32, i32) {
        (self.w - 1 - x, self.h - 1 - y)
    }

    /// Doubled coordinates from the map's centre: the mirror cell has `(-u, -v)`.
    pub fn centred(&self, x: i32, y: i32) -> (i32, i32) {
        (2 * x - (self.w - 1), 2 * y - (self.h - 1))
    }

    pub fn is_water(&self, x: i32, y: i32) -> bool {
        self.inside(x, y) && self.get(x, y) == WATER
    }

    pub fn passable(&self, x: i32, y: i32) -> bool {
        self.inside(x, y) && TILES[self.get(x, y) as usize].passable
    }

    /// Open ground that woods, ore and landmarks may take.
    pub fn is_plain(&self, tile: u32) -> bool {
        [GRASS, DIRT, DESERT, SNOW, SAND, CROPS, ASH].contains(&tile)
    }

    /// Paints `tile` on every cell for which `inside(x, y)` holds.
    pub fn paint(&mut self, inside: impl Fn(&Painter, i32, i32) -> bool, tile: u32) {
        self.fill(inside, |_, _, _| tile);
    }

    /// A noisy blob of `tile` around (cx, cy) and its mirror that only takes open ground:
    /// water, roads, towns, woods and ore stay as they are.
    pub fn patch(&mut self, cx: i32, cy: i32, r: i32, rough: i32, seed: u32, tile: u32) {
        self.blob(cx, cy, r, rough, seed, |p, x, y| {
            if p.is_plain(p.get(x, y)) {
                tile
            } else {
                p.get(x, y)
            }
        });
    }

    /// Paints `tile` on every cell for which `inside(x, y)` holds.
    pub fn fill(
        &mut self,
        inside: impl Fn(&Painter, i32, i32) -> bool,
        tile: impl Fn(&Painter, i32, i32) -> u32,
    ) {
        for y in 0..self.h {
            for x in 0..self.w {
                if inside(self, x, y) {
                    let t = tile(self, x, y);
                    self.set(x, y, t);
                }
            }
        }
    }

    /// The local forest on plain cells where the noise exceeds `threshold`, within `region`.
    pub fn forests(
        &mut self,
        scale: i32,
        seed: u32,
        threshold: i64,
        region: impl Fn(i32, i32) -> bool,
    ) {
        self.fill(
            |p, x, y| {
                region(x, y) && p.is_plain(p.get(x, y)) && p.noise(x, y, scale, seed) > threshold
            },
            |p, x, y| p.biome(x, y).forest,
        );
    }

    /// A noisy blob of `tile` around (cx, cy) and its mirror, with radius `r` and edge roughness
    /// `rough` (cells).
    pub fn blob(
        &mut self,
        cx: i32,
        cy: i32,
        r: i32,
        rough: i32,
        seed: u32,
        tile: impl Fn(&Painter, i32, i32) -> u32,
    ) {
        let (mx, my) = self.mirror_of(cx, cy);
        self.blob_once(cx, cy, r, rough, seed, &tile);
        self.blob_once(mx, my, r, rough, seed, &tile);
    }

    fn blob_once(
        &mut self,
        cx: i32,
        cy: i32,
        r: i32,
        rough: i32,
        seed: u32,
        tile: &impl Fn(&Painter, i32, i32) -> u32,
    ) {
        let reach = r + rough + 1;
        for y in cy - reach..=cy + reach {
            for x in cx - reach..=cx + reach {
                if !self.inside(x, y) {
                    continue;
                }
                let radius = i64::from(r + self.wob(x, y, 6, seed, rough));
                let d2 = i64::from((x - cx).pow(2) + (y - cy).pow(2));
                if d2 <= radius * radius + radius {
                    let t = tile(self, x, y);
                    self.set(x, y, t);
                }
            }
        }
    }

    /// An ore field and its mirror: the local ore on open ground of a blob.
    pub fn ore(&mut self, cx: i32, cy: i32, r: i32, seed: u32) {
        let (mx, my) = self.mirror_of(cx, cy);
        self.ore_once(cx, cy, r, seed);
        self.ore_once(mx, my, r, seed);
    }

    fn ore_once(&mut self, cx: i32, cy: i32, r: i32, seed: u32) {
        let reach = r + 2;
        for y in cy - reach..=cy + reach {
            for x in cx - reach..=cx + reach {
                // Ore clears trees, but not water, rock, roads or buildings. In a marsh it lies
                // in the mud.
                if !self.inside(x, y) {
                    continue;
                }
                let tile = self.get(x, y);
                if !(self.is_plain(tile) || is_woods(tile) || tile == MARSH) {
                    continue;
                }
                let radius = i64::from(r + self.wob(x, y, 4, seed, 1));
                if i64::from((x - cx).pow(2) + (y - cy).pow(2)) <= radius * radius + radius {
                    let ore = if [MARSH, MANGROVE].contains(&tile) {
                        MARSH_ORE
                    } else {
                        self.biome(x, y).ore
                    };
                    self.set(x, y, ore);
                }
            }
        }
    }

    /// A road cell: a bridge over water, a street in a city, else the local road.
    fn road_tile(&self, x: i32, y: i32) -> u32 {
        match self.get(x, y) {
            WATER | BRIDGE => BRIDGE,
            tile if is_urban(tile) => STREET,
            // Farm tracks through the fields; a causeway of earth through a marsh.
            DIRT | CROPS | TRACK | MARSH | ORCHARD | FARMHOUSE => TRACK,
            _ => self.biome(x, y).road,
        }
    }

    /// A straight road along x (fixed y) of `width` cells, from x0 to x1 inclusive, and its
    /// mirror.
    pub fn road_x(&mut self, y: i32, x0: i32, x1: i32, width: i32) {
        self.road_x_once(y, x0, x1, width);
        let (mx0, my) = self.mirror_of(x0, y + width - 1);
        let (mx1, _) = self.mirror_of(x1, y);
        self.road_x_once(my, mx0, mx1, width);
    }

    fn road_x_once(&mut self, y: i32, x0: i32, x1: i32, width: i32) {
        for x in x0.min(x1)..=x0.max(x1) {
            for k in 0..width {
                if self.inside(x, y + k) {
                    let t = self.road_tile(x, y + k);
                    self.set(x, y + k, t);
                }
            }
        }
    }

    /// A straight road along y (fixed x) of `width` cells, from y0 to y1 inclusive, and its
    /// mirror.
    pub fn road_y(&mut self, x: i32, y0: i32, y1: i32, width: i32) {
        self.road_y_once(x, y0, y1, width);
        let (mx, my0) = self.mirror_of(x + width - 1, y0);
        let (_, my1) = self.mirror_of(x, y1);
        self.road_y_once(mx, my0, my1, width);
    }

    fn road_y_once(&mut self, x: i32, y0: i32, y1: i32, width: i32) {
        for y in y0.min(y1)..=y0.max(y1) {
            for k in 0..width {
                if self.inside(x + k, y) {
                    let t = self.road_tile(x + k, y);
                    self.set(x + k, y, t);
                }
            }
        }
    }

    /// An L-shaped road from (x0, y0) along x, then along y to (x1, y1).
    pub fn road_l(&mut self, (x0, y0): (i32, i32), (x1, y1): (i32, i32), width: i32) {
        self.road_x(y0, x0, x1, width);
        self.road_y(x1, y0, y1, width);
    }

    /// A hamlet around (x, y) and its mirror: the local house on every third open cell within
    /// `r` cells.
    pub fn village(&mut self, x: i32, y: i32, r: i32, seed: u32) {
        let (mx, my) = self.mirror_of(x, y);
        for (cx, cy) in [(x, y), (mx, my)] {
            for yy in cy - r..=cy + r {
                for xx in cx - r..=cx + r {
                    let canonical = if yy < self.h / 2 {
                        (xx, yy)
                    } else {
                        self.mirror_of(xx, yy)
                    };
                    if self.inside(xx, yy)
                        && (xx - cx).pow(2) + (yy - cy).pow(2) <= r * r + r
                        && self.is_plain(self.get(xx, yy))
                        && hash(canonical.0, canonical.1, seed).is_multiple_of(3)
                    {
                        let house = self.biome(xx, yy).house;
                        self.set(xx, yy, house);
                    }
                }
            }
        }
    }

    /// A city over the cells where `region` holds: a street grid of `period` cells with streets
    /// two cells wide, aligned on the map's centre (so a city there is point-symmetric). Blocks
    /// within `downtown` cells of (cx, cy) grow towers; some blocks are parks or plazas.
    #[allow(clippy::too_many_arguments)]
    pub fn city(
        &mut self,
        region: impl Fn(&Painter, i32, i32) -> bool,
        cx: i32,
        cy: i32,
        downtown: i32,
        period: i32,
        seed: u32,
    ) {
        for y in 0..self.h {
            for x in 0..self.w {
                if !region(self, x, y) || self.is_water(x, y) {
                    continue;
                }
                let (u, v) = self.centred(x, y);
                let avenue = self.avenue;
                let street = |c: i32| {
                    let m = c.rem_euclid(2 * period);
                    m <= avenue || m >= 2 * period - avenue
                };
                if street(u) || street(v) {
                    self.set(x, y, STREET);
                    continue;
                }
                // The block, the same for both mirror cells.
                let (bu, bv) = (
                    (u + period).div_euclid(2 * period),
                    (v + period).div_euclid(2 * period),
                );
                let key = if (bv, bu) < (-bv, -bu) {
                    (bu, bv)
                } else {
                    (-bu, -bv)
                };
                let roll = hash(key.0, key.1, seed) % 100;
                let d2 = (x - cx).pow(2) + (y - cy).pow(2);
                let core = d2 <= downtown * downtown;
                // The block's outer ring of cells faces a street.
                let tile = if roll < 9 {
                    // Park: trees on lawn (or what grows here).
                    if hash(x.min(self.w - 1 - x), y.min(self.h - 1 - y), seed ^ 5)
                        .is_multiple_of(3)
                    {
                        self.biome(x, y).forest
                    } else {
                        self.biome(x, y).plain
                    }
                } else if roll < 15 {
                    PLAZA
                } else if core && hash(key.0, key.1, seed ^ 11) % 10 < 7 {
                    self.biome(x, y).tower
                } else {
                    self.biome(x, y).block
                };
                self.set(x, y, tile);
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // Finishing passes.

    /// Sandy beaches: plain land next to water becomes the local shore (`width` cells deep,
    /// thinner where the noise says so).
    pub fn beaches(&mut self, width: i32, seed: u32) {
        let near_water = |p: &Painter, x: i32, y: i32, d: i32| {
            (-d..=d)
                .any(|dy| (-d..=d).any(|dx| dx.abs() + dy.abs() <= d && p.is_water(x + dx, y + dy)))
        };
        let mut changes = Vec::new();
        for y in 0..self.h {
            for x in 0..self.w {
                let tile = self.get(x, y);
                if ![GRASS, DESERT, DIRT, CROPS].contains(&tile) {
                    continue;
                }
                let d = if self.noise(x, y, 5, seed) > 560 {
                    width
                } else {
                    1
                };
                if near_water(self, x, y, d) {
                    changes.push((x, y, self.biome(x, y).shore));
                }
            }
        }
        for (x, y, t) in changes {
            self.set(x, y, t);
        }
    }

    /// Every start gets clear building ground around its yard, `apron` cells beyond it, and a
    /// road from the yard's front to the nearest road.
    pub fn base_sites(&mut self, apron: i32) {
        for (sx, sy) in self.starts.clone() {
            for y in sy - apron..=sy + 2 + apron {
                for x in sx - apron..=sx + 2 + apron {
                    if !self.inside(x, y) {
                        continue;
                    }
                    let corner = (x - (sx + 1)).abs() + (y - (sy + 1)).abs() > apron + 4;
                    if corner {
                        continue;
                    }
                    // The yard and the ring around it are always building ground; the rest of
                    // the apron keeps its water and ore.
                    let yard = (sx - 2..=sx + 4).contains(&x) && (sy - 2..=sy + 4).contains(&y);
                    let t = self.get(x, y);
                    if yard || (t != WATER && !TILES[t as usize].ore.is_positive()) {
                        let plain = self.biome(x, y).plain;
                        self.set(x, y, plain);
                    }
                }
            }
        }
    }

    /// Passable cells that cannot be reached from the first start (pockets behind trees or
    /// buildings) are filled with the local forest, so that every passable cell is reachable.
    pub fn fill_pockets(&mut self) {
        let (sx, sy) = self.starts[0];
        let mut seen = vec![false; self.tiles.len()];
        let mut queue = std::collections::VecDeque::from([(sx, sy)]);
        seen[(sy * self.w + sx) as usize] = true;
        while let Some((x, y)) = queue.pop_front() {
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                if self.passable(nx, ny) && !seen[(ny * self.w + nx) as usize] {
                    seen[(ny * self.w + nx) as usize] = true;
                    queue.push_back((nx, ny));
                }
            }
        }
        for y in 0..self.h {
            for x in 0..self.w {
                if self.passable(x, y) && !seen[(y * self.w + x) as usize] {
                    let tile = match self.get(x, y) {
                        PLAZA | STREET => self.biome(x, y).block,
                        // A floe adrift is open water to the eye and to ships.
                        BRIDGE | ICE => WATER,
                        MARSH | MARSH_ORE => MANGROVE,
                        _ => self.biome(x, y).forest,
                    };
                    self.set(x, y, tile);
                }
            }
        }
    }

    /// Rows `H/2..H` become the point-mirror of rows `0..H/2`.
    pub fn mirror(&mut self) {
        for y in self.h / 2..self.h {
            for x in 0..self.w {
                let t = self.get(self.w - 1 - x, self.h - 1 - y);
                self.set(x, y, t);
            }
        }
    }

    /// The shared finishing passes, in order.
    pub fn finish(&mut self, beach: i32, seed: u32) {
        self.mirror();
        self.beaches(beach, seed);
        self.base_sites(6);
        self.mirror();
        self.fill_pockets();
        self.mirror();
    }

    // -----------------------------------------------------------------------------------------
    // Tools for the country between the bases.

    /// Two ore fields of radius `r` beside every base, where the ground around it has the most
    /// room for them: nine cells from the yard, on different sides of it.
    pub fn home_ore(&mut self, r: i32, seed: u32) {
        // The fields are chosen on the map as it will be, both halves alike.
        self.mirror();
        let homes: Vec<(i32, i32)> = self.starts.iter().copied().step_by(2).collect();
        for (k, (sx, sy)) in homes.into_iter().enumerate() {
            let (cx, cy) = (sx + 1, sy + 1);
            let room = |p: &Painter, (dx, dy): (i32, i32)| -> i32 {
                let mut cells = 0;
                for y in cy + dy - r..=cy + dy + r {
                    for x in cx + dx - r..=cx + dx + r {
                        let d2 = (x - cx - dx).pow(2) + (y - cy - dy).pow(2);
                        if p.inside(x, y)
                            && d2 <= r * r + r
                            && (p.is_plain(p.get(x, y)) || is_woods(p.get(x, y)))
                        {
                            cells += 1;
                        }
                    }
                }
                cells
            };
            let rooms: Vec<i32> = RING.iter().map(|&d| room(self, d)).collect();
            // The first of the best, so that the choice does not depend on anything but the map.
            let best = |allowed: &dyn Fn(usize) -> bool| {
                (0..RING.len())
                    .filter(|&i| allowed(i))
                    .max_by_key(|&i| (rooms[i], std::cmp::Reverse(i)))
            };
            let Some(first) = best(&|_| true) else {
                continue;
            };
            let second = best(&|i| {
                let apart = (i + RING.len() - first) % RING.len();
                (5..=11).contains(&apart)
            });
            for (n, i) in [Some(first), second].into_iter().flatten().enumerate() {
                let (dx, dy) = RING[i];
                self.ore(cx + dx, cy + dy, r, seed + (2 * k + n) as u32);
            }
        }
    }

    /// Landmarks scattered over the open country, one to every `spacing` x `spacing` cells or
    /// so where `region` holds and no base is within `keep` cells: copses, ponds, crags, hamlets,
    /// farms, ruins, marshes, small ore fields, whatever belongs to the environment there. They
    /// only take open ground, so no road, river, town or ore field is touched, and they are
    /// small, so they shape a fight without walling anything off.
    pub fn landmarks(
        &mut self,
        spacing: i32,
        keep: i32,
        seed: u32,
        region: impl Fn(&Painter, i32, i32) -> bool,
    ) {
        self.mirror();
        for gy in 0..=self.h / 2 / spacing {
            for gx in 0..=self.w / spacing {
                let roll = hash(gx, gy, seed);
                let x = gx * spacing + (roll % spacing as u32) as i32;
                let y = gy * spacing + ((roll >> 8) % spacing as u32) as i32;
                if y >= self.h / 2
                    || !self.inside(x, y)
                    || !self.is_plain(self.get(x, y))
                    || self.near_start(x, y, keep)
                    || !region(self, x, y)
                {
                    continue;
                }
                self.landmark(x, y, roll >> 16);
            }
        }
    }

    // The kinds are listed one by one where a range would end on another arm's first kind.
    #[allow(clippy::manual_range_patterns)]
    fn landmark(&mut self, x: i32, y: i32, roll: u32) {
        let seed = 0x6000 + (roll & 0xff);
        let size = (roll >> 8) as i32 % 2;
        let biome = self.biome(x, y);
        let kind = (roll >> 4) % 16;
        match (biome.plain, kind) {
            // Everywhere: a wood, a crag, a hamlet, a small ore field, ruins of a farm or post.
            (_, 0..=2) => self.patch(x, y, 2 + size, 2, seed, biome.forest),
            (_, 3) => self.patch(x, y, 1 + size, 1, seed, biome.cliff),
            (_, 4) => self.village(x, y, 3, seed),
            (_, 5 | 6) => self.ore(x, y, 2, seed),
            (_, 7) => {
                self.patch(x, y, 2, 1, seed, PLAZA);
                for (dx, dy) in [(-1, -1), (1, 0), (-1, 1)] {
                    for (cx, cy) in [(x + dx, y + dy), self.mirror_of(x + dx, y + dy)] {
                        if self.inside(cx, cy) && self.get(cx, cy) == PLAZA {
                            self.set(cx, cy, biome.block);
                        }
                    }
                }
            }
            // Snow: frozen ponds and tarns.
            (SNOW, 8 | 9 | 10) => self.patch(x, y, 3 + size, 2, seed, ICE),
            (SNOW, 11) => self.patch(x, y, 2, 1, seed, WATER),
            // Desert: an oasis (a pool ringed by palms in a patch of earth), dry washes.
            (DESERT, 8 | 9) => {
                self.patch(x, y, 5, 2, seed, DIRT);
                self.blob(x, y, 3, 1, seed + 1, |p, x, y| {
                    if p.get(x, y) == DIRT {
                        PALMS
                    } else {
                        p.get(x, y)
                    }
                });
                self.blob(x, y, 1, 1, seed + 2, |p, x, y| {
                    if p.get(x, y) == PALMS {
                        WATER
                    } else {
                        p.get(x, y)
                    }
                });
            }
            (DESERT, 10 | 11) => self.patch(x, y, 3 + size, 2, seed, DIRT),
            // Ash: vents ringed by basalt, hot springs.
            (ASH, 8 | 9 | 10) => {
                self.patch(x, y, 3, 1, seed, BASALT);
                self.blob(x, y, 1 + size, 1, seed + 1, |p, x, y| {
                    if p.get(x, y) == BASALT {
                        LAVA
                    } else {
                        p.get(x, y)
                    }
                });
            }
            (ASH, 11) => self.patch(x, y, 2, 1, seed, WATER),
            (SNOW | DESERT | ASH, _) => {}
            // Green country: ponds in their marsh, farms in their fields, orchards.
            (_, 8 | 9) => {
                self.patch(x, y, 4 + size, 2, seed, MARSH);
                self.blob(x, y, 2 + size, 1, seed + 1, |p, x, y| {
                    if p.get(x, y) == MARSH {
                        WATER
                    } else {
                        p.get(x, y)
                    }
                });
            }
            (_, 10 | 11 | 12) => {
                self.patch(x, y, 5 + size, 2, seed, CROPS);
                for (dx, dy) in [(0, 0), (1, 0), (0, 2), (-2, 1)] {
                    for (cx, cy) in [(x + dx, y + dy), self.mirror_of(x + dx, y + dy)] {
                        if self.inside(cx, cy) && self.get(cx, cy) == CROPS {
                            self.set(cx, cy, FARMHOUSE);
                        }
                    }
                }
            }
            (_, 13) => self.patch(x, y, 2 + size, 1, seed, ORCHARD),
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The maps. Start positions (the yards' footprint origins) are listed in `gen_maps.rs`.
//
// Every recipe paints, in this order: the lie of the land (seas, rivers, mountains, towns), the
// landmarks of the open country, the roads (which cut through whatever a landmark put in their
// way, and bridge its ponds), the ore, and the shared finishing passes.

fn temperate(_: &Painter, _: i32, _: i32) -> Biome {
    TEMPERATE
}

/// Squared distance from the map's centre, in cells.
pub fn centre_d2(p: &Painter, x: i32, y: i32) -> i32 {
    let (u, v) = p.centred(x, y);
    (u * u + v * v) / 4
}

/// The key of a cell that its point mirror shares: for hashes that must fall alike on both
/// halves of a map.
pub fn canonical(p: &Painter, x: i32, y: i32) -> (i32, i32) {
    if y < p.h / 2 {
        (x, y)
    } else {
        p.mirror_of(x, y)
    }
}

/// Farmland over the green country: ploughed earth where the noise is high, and among it fields
/// in crop.
fn farmland(p: &mut Painter, scale: i32, seed: u32, threshold: i64) {
    p.paint(
        |p, x, y| p.get(x, y) == GRASS && p.noise(x, y, scale, seed) > threshold,
        DIRT,
    );
    p.paint(
        |p, x, y| p.get(x, y) == DIRT && p.noise(x, y, 7, seed ^ 0x51) > 500,
        CROPS,
    );
}

/// Liberty Bay, 256 x 256: a metropolis on a river between two bays. Bases lie in green
/// countryside around it; the river runs through downtown from bay to bay between marshy banks
/// and is crossed by the avenues' bridges and a country bridge on each side. A port town with
/// piers stands on each bay.
pub fn liberty_bay(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    let (cx, cy) = (w / 2, h / 2);
    // Bays in the north-east and south-west corners, with rocky headlands.
    p.paint(
        |p, x, y| (x - y).abs() > 148 + p.wob(x, y, 36, 1, 16) + p.wob(x, y, 9, 31, 3),
        WATER,
    );
    p.paint(
        |p, x, y| !p.is_water(x, y) && (x - y).abs() > 130 && p.noise(x, y, 13, 2) > 650,
        ROCK,
    );
    // Farmland and woods.
    farmland(p, 16, 3, 630);
    p.forests(19, 4, 610, |_, _| true);
    // Suburbs: cottages and gardens around downtown.
    p.paint(
        |p, x, y| {
            let d = centre_d2(p, x, y);
            let (kx, ky) = canonical(p, x, y);
            (54 * 54..=84 * 84).contains(&d)
                && p.is_plain(p.get(x, y))
                && p.noise(x, y, 9, 33) > 470
                && hash(kx, ky, 7).is_multiple_of(5)
        },
        COTTAGE,
    );
    // Downtown: blocks around the centre with towers in its core.
    p.city(
        |p, x, y| centre_d2(p, x, y) <= (50 + p.wob(x, y, 10, 5, 4)).pow(2),
        cx,
        cy,
        26,
        6,
        6,
    );
    // The port towns on the bays.
    p.city(
        |p, x, y| {
            let d = |cx: i32, cy: i32| (x - cx).pow(2) + (y - cy).pow(2) <= 11 * 11;
            let (mx, my) = p.mirror_of(184, 58);
            d(184, 58) || d(mx, my)
        },
        184,
        58,
        4,
        5,
        35,
    );
    // The river: from bay to bay through downtown, with marshes on its banks in the country.
    let off = move |p: &Painter, x: i32, y: i32| (x + y - (w - 1) + p.wob(x, y, 32, 8, 10)).abs();
    p.paint(|p, x, y| off(p, x, y) <= 4, WATER);
    p.paint(
        |p, x, y| {
            (5..=9).contains(&off(p, x, y))
                && p.is_plain(p.get(x, y))
                && centre_d2(p, x, y) > 62 * 62
                && p.noise(x, y, 9, 34) > 440
        },
        MARSH,
    );
    // Lakes and a sheltering ridge in the countryside.
    p.blob(44, 88, 8, 4, 9, |_, _, _| WATER);
    p.blob(88, 52, 6, 4, 10, |_, _, _| WATER);
    p.blob(64, 60, 4, 3, 11, |_, _, _| ROCK);
    p.landmarks(22, 16, 36, |p, x, y| centre_d2(p, x, y) > 60 * 60);
    // Roads: avenues through downtown (streets and bridges there), country roads to the bases.
    p.road_x(cy - 1, 8, w - 9, 2);
    p.road_y(cx - 1, 8, h - 9, 2);
    p.road_x(39, 37, cx, 2);
    p.road_y(39, 37, cy, 2);
    p.road_x(81, 40, 100, 2);
    p.road_y(81, 40, 100, 2);
    // Country bridges over the river outside downtown, and the roads to the ports and piers.
    p.road_x(69, cx - 1, 200, 2);
    p.road_y(200, 69, 104, 2);
    p.road_x(57, 150, 226, 2);
    p.road_y(183, 28, 69, 2);
    p.road_x(153, 30, 81, 2);
    // Ore: fields at each base, contested fields in the suburbs and by the bays.
    for (x, y, r, s) in [
        (13, 93, 4, 22),
        (101, 11, 4, 23),
        (69, 109, 5, 24),
        (109, 69, 5, 25),
        (45, 129, 5, 26),
        (129, 45, 5, 27),
        (212, 76, 4, 28),
        (140, 20, 4, 29),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 500);
    p.finish(2, 30);
}

fn arctic_biome(p: &Painter, x: i32, y: i32) -> Biome {
    // Thawed valleys follow the two side passes.
    let across = (x - y).abs();
    if (56..=88).contains(&(across + p.wob(x, y, 20, 40, 8))) {
        TEMPERATE
    } else {
        ARCTIC
    }
}

/// Frostline, 224 x 224: snowfields and pine forests on both sides of a mountain chain, an arctic
/// sea on the west and east coasts, thawed valleys through the side passes, frozen lakes, and an
/// industrial town astride the central pass.
pub fn frostline(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    let (cx, cy) = (w / 2, h / 2);
    // Arctic sea along the west and east edges, with fjords and pack ice along the shore.
    let edge = |p: &Painter, x: i32| x.min(p.w - 1 - x);
    p.paint(
        |p, x, y| edge(p, x) < 12 + p.wob(x, y, 28, 41, 8) + p.wob(x, y, 9, 42, 2),
        WATER,
    );
    p.paint(
        |p, x, y| edge(p, x) < 26 && p.noise(x, y, 10, 43) > 700,
        WATER,
    );
    p.paint(
        |p, x, y| p.is_water(x, y) && edge(p, x) > 5 && p.noise(x, y, 8, 53) > 640,
        ICE,
    );
    // The mountain chain along the anti-diagonal, broken by three passes.
    p.fill(
        |p, x, y| {
            let ridge = (x + y - (p.w - 1)).abs() <= 5 + p.wob(x, y, 16, 44, 4);
            let across = x - y;
            let pass = across.abs() <= 7 || (across.abs() - 72).abs() <= 7;
            ridge && !pass
        },
        |p, x, y| {
            if p.noise(x, y, 6, 45) > 720 {
                p.biome(x, y).forest
            } else {
                p.biome(x, y).cliff
            }
        },
    );
    // A frozen lake, a mountain lake, farmland in the thawed valleys, and pines.
    p.blob(52, 88, 9, 4, 46, |_, _, _| ICE);
    p.blob(84, 52, 6, 4, 47, |_, _, _| WATER);
    farmland(p, 14, 54, 650);
    p.forests(18, 48, 600, |_, _| true);
    // The town at the central pass, and hamlets.
    p.city(
        |p, x, y| centre_d2(p, x, y) <= (24 + p.wob(x, y, 10, 49, 4)).pow(2),
        cx,
        cy,
        10,
        5,
        50,
    );
    for (x, y) in [(73, 61), (93, 89), (29, 69), (150, 30), (120, 70)] {
        p.village(x, y, 4, 51);
    }
    // A river from the mountains to the sea.
    p.paint(
        |p, x, y| y < 80 && x > 12 && (x - 2 * y / 3 - 6 + p.wob(x, y, 20, 52, 6)).abs() <= 2,
        WATER,
    );
    p.landmarks(20, 15, 55, |p, x, y| centre_d2(p, x, y) > 30 * 30);
    // Roads.
    p.road_x(33, 17, cx, 2);
    p.road_y(cx - 1, 17, cy, 2);
    p.road_y(45, 33, cy, 2);
    p.road_x(cy - 1, 8, w - 9, 2);
    p.road_x(73, 45, 125, 2);
    // The side passes stay open, whatever grew in them.
    p.stroke_with(&[(64, 136), (88, 160)], 3, |p, x, y| p.biome(x, y).plain);
    // Ore.
    for (x, y, r, s) in [
        (89, 105, 5, 64),
        (117, 61, 4, 65),
        (29, 57, 4, 60),
        (65, 25, 4, 61),
        (170, 28, 4, 62),
        (60, 140, 4, 63),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 520);
    p.finish(1, 66);
}

fn desert_biome(p: &Painter, x: i32, y: i32) -> Biome {
    // An irrigated green belt along the coast road.
    if (y - p.h / 2).abs() < 8 + p.wob(x, y, 24, 70, 6) && x.min(p.w - 1 - x) > 40 {
        TEMPERATE
    } else {
        DESERT_BIOME
    }
}

/// Oasis Gulf, 256 x 192: desert dunes and sandstone canyons between two gulf coasts, palm
/// oases, a green belt of fields along the highway and a harbour city with towers at the centre.
pub fn oasis_gulf(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    let (cx, cy) = (w / 2, h / 2);
    // The gulf: sea along the north and south edges.
    p.paint(
        |p, x, y| y.min(p.h - 1 - y) < 12 + p.wob(x, y, 32, 71, 8) + p.wob(x, y, 9, 72, 2),
        WATER,
    );
    // Canyons: sandstone mesas in long bands, cut by washes.
    p.paint(
        |p, x, y| {
            let band = p.noise(x, y, 20, 73);
            (575..640).contains(&band) && p.noise(x, y, 36, 74) > 470 && !p.is_water(x, y)
        },
        MESA,
    );
    // Oases: lakes ringed by palms and fields.
    for (x, y, r, s) in [(48, 56, 6, 75), (80, 60, 4, 76), (164, 44, 4, 77)] {
        p.blob(x, y, r + 8, 4, s, |p, x, y| {
            if p.is_water(x, y) { WATER } else { DIRT }
        });
        p.blob(x, y, r + 4, 2, s + 1, |_, _, _| PALMS);
        p.blob(x, y, r, 2, s + 2, |_, _, _| WATER);
    }
    // The green belt is farmed.
    farmland(p, 12, 83, 520);
    p.forests(12, 78, 740, |_, _| true);
    // The harbour city.
    p.city(
        |p, x, y| {
            let (u, v) = p.centred(x, y);
            u * u / 8 + v * v / 4 <= (26 + p.wob(x, y, 12, 79, 4)).pow(2)
        },
        cx,
        cy,
        16,
        6,
        80,
    );
    for (x, y) in [(85, 25), (25, 61), (141, 33), (200, 60), (60, 84)] {
        p.village(x, y, 4, 81);
    }
    // A wadi stream to the gulf.
    p.paint(
        |p, x, y| y < 60 && (x - 94 + p.wob(x, y, 24, 82, 6)).abs() <= 2,
        WATER,
    );
    p.landmarks(20, 15, 84, |p, x, y| {
        let (u, v) = p.centred(x, y);
        u * u / 8 + v * v / 4 > 34 * 34
    });
    // Highways.
    p.road_x(cy - 1, 8, w - 9, 2);
    p.road_y(37, 41, cy, 2);
    p.road_x(45, 37, 153, 2);
    p.road_y(153, 45, cy, 2);
    p.road_y(77, 37, cy, 2);
    p.road_x(33, 153, 216, 2);
    // Ore.
    for (x, y, r, s) in [
        (89, 81, 5, 94),
        (133, 61, 5, 95),
        (193, 29, 4, 96),
        (20, 44, 4, 90),
        (232, 60, 4, 91),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 540);
    p.finish(2, 97);
}

fn tropic(_: &Painter, _: i32, _: i32) -> Biome {
    TROPIC
}

/// Coral Archipelago, 256 x 256: islands in a warm sea. Home islands in the north-west and
/// south-east, flank islands and a resort city on the central island, joined by long bridges;
/// beaches, palms, jungle and mangrove shallows everywhere.
pub fn coral_archipelago(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    let (cx, cy) = (w / 2, h / 2);
    p.paint(|_, _, _| true, WATER);
    // Islands (each with its mirror) and islets.
    for (x, y, r, seed) in [
        (48, 48, 36, 100),
        (132, 34, 20, 101),
        (34, 124, 20, 102),
        (cx, cy, 30, 103),
        (196, 60, 20, 104),
        (88, 88, 7, 105),
        (16, 196, 7, 106),
        (232, 20, 12, 113),
        (150, 84, 9, 114),
    ] {
        p.blob(x, y, r, r / 4 + 1, seed, |_, _, _| GRASS);
    }
    // Mangrove shallows on the sheltered shores.
    p.paint(
        |p, x, y| {
            p.is_water(x, y)
                && p.noise(x, y, 10, 115) > 590
                && (-3..=3).any(|d| {
                    (p.inside(x + d, y) && p.get(x + d, y) == GRASS)
                        || (p.inside(x, y + d) && p.get(x, y + d) == GRASS)
                })
        },
        MARSH,
    );
    p.paint(
        |p, x, y| p.get(x, y) == MARSH && p.noise(x, y, 4, 116) > 620,
        MANGROVE,
    );
    // Jungle, palm groves and rocky highlands.
    p.forests(16, 107, 600, |_, _| true);
    p.paint(
        |p, x, y| p.get(x, y) == GRASS && p.noise(x, y, 6, 108) > 740,
        PALMS,
    );
    p.blob(24, 64, 4, 2, 109, |_, _, _| ROCK);
    p.blob(64, 20, 4, 2, 110, |_, _, _| ROCK);
    // The resort city on the central island.
    p.city(
        |p, x, y| centre_d2(p, x, y) <= 22 * 22 && !p.is_water(x, y),
        cx,
        cy,
        14,
        5,
        111,
    );
    for (x, y) in [(141, 41), (29, 113), (197, 65), (70, 60), (232, 24)] {
        p.village(x, y, 4, 112);
    }
    p.landmarks(18, 15, 117, |p, x, y| centre_d2(p, x, y) > 26 * 26);
    // Causeways and bridges between the islands: broad ones, since they are the only way.
    p.road_x(cy - 3, 32, w - 33, 6);
    p.road_y(cx - 3, 32, h - 33, 6);
    p.road_x(44, 48, 132, 5);
    p.road_y(44, 48, 124, 5);
    p.road_x(48, 132, 196, 5);
    p.road_y(194, 48, 127, 5);
    p.road_y(196, 22, 49, 3);
    p.road_x(22, 196, 232, 3);
    p.road_y(150, 49, 84, 2);
    p.road_y(88, 88, 127, 2);
    // Ore.
    for (x, y, r, s) in [
        (61, 69, 4, 121),
        (104, 116, 4, 125),
        (201, 55, 4, 126),
        (234, 14, 4, 129),
        (150, 86, 4, 130),
        (88, 88, 3, 131),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 560);
    p.finish(2, 127);
}

fn continental_biome(p: &Painter, x: i32, y: i32) -> Biome {
    // North-west and south-east snow, north-east and south-west desert, temperate between.
    let n = p.wob(x, y, 24, 130, 10);
    let (sum, diff) = (x + y - (p.w - 1), x - y);
    if sum.abs() > 152 + n {
        ARCTIC
    } else if diff.abs() > 152 + n {
        DESERT_BIOME
    } else {
        TEMPERATE
    }
}

/// Continental Divide, 256 x 256: every environment on one map. Snowy highlands with frozen
/// lakes in two corners, desert with a sea inlet in the other two, temperate farmland between,
/// and a lake at the centre with an island city reached by four bridges.
pub fn continental_divide(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    let (cx, cy) = (w / 2, h / 2);
    // The central lake and its island city.
    p.paint(
        |p, x, y| centre_d2(p, x, y) <= (46 + p.wob(x, y, 18, 131, 6)).pow(2),
        WATER,
    );
    p.paint(|p, x, y| centre_d2(p, x, y) <= 24 * 24, GRASS);
    p.city(|p, x, y| centre_d2(p, x, y) <= 22 * 22, cx, cy, 14, 5, 132);
    // Sea inlets in the desert corners.
    p.paint(
        |p, x, y| (x - y).abs() > 208 + p.wob(x, y, 28, 133, 12),
        WATER,
    );
    // Mountain ridges along the snowline, with gaps.
    p.fill(
        |p, x, y| {
            let sum = (x + y - (p.w - 1)).abs();
            let line = (sum - 152 - p.wob(x, y, 24, 130, 10)).abs() <= 2;
            let gap = (x - y).abs() % 60 < 16;
            line && !gap && !p.is_water(x, y)
        },
        |p, x, y| p.biome(x, y).cliff,
    );
    farmland(p, 15, 135, 660);
    p.forests(19, 136, 640, |_, _| true);
    // Desert mesas, and frozen lakes in the highlands.
    p.paint(
        |p, x, y| (x - y).abs() > 162 && p.noise(x, y, 10, 137) > 700 && !p.is_water(x, y),
        MESA,
    );
    p.paint(
        |p, x, y| p.get(x, y) == SNOW && p.noise(x, y, 11, 139) > 720,
        ICE,
    );
    for (x, y) in [
        (61, 101),
        (101, 61),
        (29, 61),
        (193, 21),
        (133, 13),
        (160, 70),
    ] {
        p.village(x, y, 4, 138);
    }
    p.landmarks(20, 15, 148, |p, x, y| centre_d2(p, x, y) > 52 * 52);
    // Roads: four bridges onto the island, roads to every base.
    p.road_x(cy - 1, 16, w - 17, 2);
    p.road_y(cx - 1, 16, h - 17, 2);
    p.road_x(41, 37, 209, 2);
    p.road_y(41, 37, 209, 2);
    p.road_x(85, 41, cx, 2);
    // Ore.
    for (x, y, r, s) in [
        (61, 93, 5, 142),
        (117, 21, 4, 145),
        (21, 117, 4, 146),
        (150, 60, 4, 149),
        (96, 108, 4, 150),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 580);
    p.finish(2, 147);
}

/// Steel Harbor, 256 x 256: a whole-map city of boulevards. A river winds west to east through
/// the middle between two harbour basins with piers; downtown it splits around an island of
/// towers. Avenue bridges cross it, promenades line it, and the bases sit in cleared industrial
/// lots on the outskirts; parks with ponds hold the contested ore.
pub fn steel_harbor(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    let (cx, cy) = (w / 2, h / 2);
    // How far (doubled rows) a cell is from the river's line, which is its own point mirror.
    let off = |p: &Painter, x: i32, y: i32| {
        let (_, v) = p.centred(x, y);
        (v - 2 * p.meander(x, p.w, 32, 200, 12)).abs()
    };
    // Downtown the river widens around an island (doubled rows).
    let widen = |p: &Painter, x: i32, y: i32| {
        let (u, _) = p.centred(x, y);
        (60 - u.abs() / 2).clamp(0, 32) + p.wob(x, y, 14, 202, 2)
    };
    p.paint(
        |p, x, y| {
            let (off, widen) = (off(p, x, y), widen(p, x, y));
            let river = off <= 10 + widen + 2 * p.wob(x, y, 18, 201, 2);
            let island = widen > 16 && off <= widen - 12;
            // The harbour basins: half-ellipses on the west and east edges.
            let e = i64::from(x.min(p.w - 1 - x));
            let d = i64::from(off / 2);
            let harbour =
                e * e * 400 + d * d * 196 <= 313_600 + 8_000 * i64::from(p.wob(x, y, 12, 203, 6));
            (river && !island) || harbour
        },
        WATER,
    );
    // The city: boulevards four cells wide every nine cells over all the land, towers downtown.
    p.avenue = 3;
    p.city(|_, _, _| true, cx, cy, 48, 9, 204);
    // Promenades along the water.
    p.paint(
        |p, x, y| {
            [CITY, TOWER].contains(&p.get(x, y))
                && [(1, 0), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .any(|&(dx, dy)| p.is_water(x + dx, y + dy))
        },
        PLAZA,
    );
    // Industrial lots for the bases and parks for the contested ore, cleared out of the grid.
    let homes: Vec<(i32, i32)> = p.starts.iter().copied().step_by(2).collect();
    for (k, &(sx, sy)) in homes.iter().enumerate() {
        p.blob(sx + 1, sy + 1, 24, 4, 205 + k as u32, |p, x, y| {
            if p.is_water(x, y) {
                WATER
            } else if p.noise(x, y, 8, 208) > 610 {
                DIRT
            } else {
                GRASS
            }
        });
    }
    for (x, y, r, s) in [
        (80, 84, 14, 209),
        (184, 72, 12, 210),
        (64, 164, 12, 211),
        (128, 44, 10, 215),
    ] {
        p.blob(
            x,
            y,
            r,
            4,
            s,
            |p, x, y| {
                if p.is_water(x, y) { WATER } else { GRASS }
            },
        );
    }
    p.forests(8, 212, 690, |_, _| true);
    p.blob(86, 90, 4, 2, 213, |_, _, _| WATER);
    p.blob(60, 170, 2, 2, 214, |_, _, _| WATER);
    // Avenues: crossings of the river, boulevards between the districts.
    p.road_y(55, 8, h - 9, 3);
    p.road_y(99, 57, h - 58, 2);
    p.road_y(cx - 1, 28, h - 29, 2);
    p.road_x(55, 8, w - 9, 3);
    p.road_x(85, 41, w - 42, 2);
    // Piers into the harbour.
    p.road_x(113, 8, 37, 1);
    p.road_x(137, 12, 37, 1);
    // Ore: two fields in each lot, contested fields in the parks.
    for (x, y, r, s) in [
        (77, 79, 5, 226),
        (187, 69, 4, 227),
        (67, 161, 4, 228),
        (128, 42, 4, 230),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 600);
    p.finish(1, 229);
}

fn glacier_biome(p: &Painter, x: i32, y: i32) -> Biome {
    // Green valleys on the west and east, snow from the foothills up.
    let (u, _) = p.centred(x, y);
    if u.abs() < 108 + 2 * p.wob(x, y, 20, 240, 12) {
        ARCTIC
    } else {
        TEMPERATE
    }
}

/// Glacier Pass, 240 x 240: green valleys with farms, woods and a fjord on the west and east,
/// separated by a snowy massif. The massif is crossed by a winding pass in the north and the
/// south, a narrow goat trail on each side, and a wide central pass around a glacial lake rimmed
/// with ice, with a causeway and a mining hamlet; every crossing is a long chokepoint.
pub fn glacier_pass(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    // The coast, and a fjord into each valley (the western one; the finishing mirror makes the
    // eastern).
    p.paint(
        |p, x, y| {
            let fjord =
                y < p.h / 2 && x < 40 && (y - 62 - p.wob(x, y, 16, 241, 4)).abs() <= 6 - x / 6;
            fjord || x.min(p.w - 1 - x) < 6 + p.wob(x, y, 20, 242, 4)
        },
        WATER,
    );
    // The massif.
    p.fill(
        |p, x, y| {
            let (u, _) = p.centred(x, y);
            u.abs() <= 66 + 2 * p.wob(x, y, 28, 243, 14)
        },
        |p, x, y| {
            if p.noise(x, y, 6, 244) > 690 {
                SNOW_PINES
            } else {
                SNOW_ROCK
            }
        },
    );
    // Valleys: farmland and woods; pines on the snowy foothills.
    farmland(p, 14, 249, 640);
    p.forests(17, 250, 620, |_, _| true);
    p.forests(8, 251, 600, |x, _| (68..128).contains(&(2 * x - 239).abs()));
    // The northern pass and goat trail (painted with their southern mirrors).
    p.stroke(
        &[
            (68, 44),
            (84, 34),
            (104, 42),
            (128, 28),
            (152, 38),
            (172, 30),
        ],
        6,
        SNOW,
    );
    p.stroke(
        &[(68, 80), (90, 70), (114, 80), (138, 68), (172, 78)],
        3,
        SNOW,
    );
    // The central pass, its crags and glacial lake, and the mining hamlet.
    p.fill(
        |p, x, y| {
            let (u, v) = p.centred(x, y);
            u.abs() <= 92 && v.abs() <= 50 + 2 * p.wob(x, y, 16, 245, 6)
        },
        |p, x, y| p.biome(x, y).plain,
    );
    for (x, y, r, s) in [(88, 104, 4, 246), (140, 98, 3, 247)] {
        p.blob(x, y, r, 2, s, |_, _, _| SNOW_ROCK);
    }
    p.blob(119, 119, 17, 2, 248, |_, _, _| ICE);
    p.blob(119, 119, 11, 2, 253, |_, _, _| WATER);
    for (x, y) in [
        (53, 51),
        (57, 91),
        (105, 101),
        (193, 65),
        (40, 150),
        (30, 20),
    ] {
        p.village(x, y, 4, 252);
    }
    p.landmarks(20, 15, 254, |p, x, y| {
        let (u, _) = p.centred(x, y);
        // The valleys and the central pass, not the trails through the massif.
        u.abs() > 140 || (y - p.h / 2).abs() < 20
    });
    // Roads: to the northern pass, over the lake by a causeway, and along the valleys.
    p.road_x(35, 29, 68, 2);
    p.road_x(33, 172, 208, 2);
    p.road_x(h / 2 - 1, 8, w - 9, 2);
    p.road_y(45, 35, h - 36, 2);
    // Ore: two fields at each base, contested fields at the passes.
    for (x, y, r, s) in [
        (80, 100, 4, 266),
        (116, 36, 4, 267),
        (128, 96, 4, 268),
        (40, 70, 4, 270),
        (36, 168, 4, 271),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 620);
    p.finish(1, 269);
}

fn desert(_: &Painter, _: i32, _: i32) -> Biome {
    DESERT_BIOME
}

/// Red Canyon, 192 x 192: a sandstone plateau cut by canyons. Each base lies in a basin of its
/// own; a canyon runs from every basin to a central basin with a ruined adobe town around an
/// oasis pool, and long winding canyons join neighbouring basins around the rim (the flanking
/// routes), past palm oases in side basins.
pub fn red_canyon(p: &mut Painter) {
    let (cx, cy) = (p.w / 2, p.h / 2);
    p.paint(|_, _, _| true, MESA);
    // Basins: one per base, a central one and oases in side basins.
    for (x, y, r, s) in [
        (23, 23, 26, 280),
        (19, 103, 24, 281),
        (103, 19, 24, 282),
        (cx, cy, 32, 283),
        (11, 61, 10, 284),
        (45, 149, 10, 285),
        (62, 62, 11, 304),
        (150, 44, 22, 305),
    ] {
        p.blob(x, y, r, 4, s, |_, _, _| DESERT);
    }
    // Canyons: spokes from the basins to the centre, and rim canyons between neighbours.
    for (points, r) in [
        (&[(28, 28), (44, 36), (56, 54), (80, 80)][..], 6),
        (&[(32, 100), (50, 88), (68, 94)][..], 6),
        (&[(100, 32), (88, 50), (94, 68)][..], 6),
        (&[(12, 36), (8, 60), (16, 80)][..], 4),
        (&[(36, 12), (62, 8), (84, 14)][..], 4),
        (
            &[(20, 120), (32, 140), (52, 156), (68, 172), (80, 176)][..],
            4,
        ),
        (&[(120, 22), (136, 34), (150, 44)][..], 5),
        (&[(150, 44), (138, 62), (118, 78)][..], 5),
        (&[(160, 58), (176, 80), (170, 104)][..], 4),
    ] {
        p.stroke(points, r, DESERT);
    }
    // Ragged canyon walls and fallen boulders.
    for round in 0..2 {
        let ragged: Vec<(i32, i32)> = (0..p.h)
            .flat_map(|y| (0..p.w).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                p.get(x, y) == MESA
                    && [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| {
                        p.inside(x + dx, y + dy) && p.get(x + dx, y + dy) == DESERT
                    })
                    && p.noise(x, y, 4, 286 + round) > 600
            })
            .collect();
        for (x, y) in ragged {
            p.set(x, y, DESERT);
        }
    }
    p.paint(
        |p, x, y| p.get(x, y) == DESERT && !p.near_start(x, y, 9) && p.noise(x, y, 2, 287) > 860,
        MESA,
    );
    // Oases.
    for (x, y, s) in [(11, 61, 288), (45, 149, 289), (62, 62, 306)] {
        p.blob(x, y, 5, 2, s, |_, _, _| PALMS);
        p.blob(x, y, 2, 1, s + 1, |_, _, _| WATER);
    }
    // The ruined town: whole blocks fallen to rubble or sand, adobe huts among them, sand
    // drifting over the streets, and the pool at its heart.
    let period = 6;
    p.city(
        |p, x, y| centre_d2(p, x, y) <= 24 * 24,
        cx,
        cy,
        10,
        period,
        290,
    );
    p.fill(
        |p, x, y| {
            [DESERT_CITY, DESERT_TOWER, STREET].contains(&p.get(x, y))
                && centre_d2(p, x, y) <= 24 * 24
        },
        |p, x, y| {
            let (u, v) = p.centred(x, y);
            let (bu, bv) = (
                (u + period).div_euclid(2 * period),
                (v + period).div_euclid(2 * period),
            );
            let key = if (bv, bu) < (-bv, -bu) {
                (bu, bv)
            } else {
                (-bu, -bv)
            };
            let (kx, ky) = canonical(p, x, y);
            let cell = hash(kx, ky, 291) % 6;
            if p.get(x, y) == STREET {
                return if cell < 2 { DESERT } else { STREET };
            }
            match hash(key.0, key.1, 292) % 10 {
                0..=2 => [PLAZA, PLAZA, ADOBE, DESERT, PLAZA, DESERT][cell as usize],
                3 | 4 => [ADOBE, DESERT, DESERT, ADOBE, DESERT, DESERT][cell as usize],
                _ => p.get(x, y),
            }
        },
    );
    p.paint(|p, x, y| centre_d2(p, x, y) <= 4 * 4 + 4, WATER);
    for (x, y) in [(57, 21), (21, 57), (150, 40)] {
        p.village(x, y, 3, 293);
    }
    p.landmarks(18, 14, 307, |p, x, y| {
        centre_d2(p, x, y) > 28 * 28
            // In the basins, not in the canyons between them.
            && (-5..=5).all(|d| {
                p.inside(x + d, y)
                    && p.inside(x, y + d)
                    && p.get(x + d, y) != MESA
                    && p.get(x, y + d) != MESA
            })
    });
    // Ore: two fields at each base, contested fields at the oases and in the central basin.
    for (x, y, r, s) in [
        (73, 81, 4, 300),
        (15, 65, 3, 301),
        (41, 155, 3, 302),
        (66, 58, 3, 308),
        (112, 60, 3, 309),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 640);
    p.finish(1, 303);
}

/// Jungle Delta, 256 x 224: a river delta in the tropics. A great lagoon with jungle islets fills
/// the centre; distributaries run from it to the northern and southern sea and to the west and
/// east coasts, cutting the land into jungle islands joined by sand bars and timber bridges.
/// Marsh and mangrove line the channels; fishing villages stand on the shores and two river
/// ports on the lagoon.
pub fn jungle_delta(p: &mut Painter) {
    // The sea along the north and south.
    p.paint(
        |p, x, y| y.min(p.h - 1 - y) < 8 + p.wob(x, y, 28, 310, 6) + p.wob(x, y, 8, 330, 2),
        WATER,
    );
    // The lagoon.
    p.paint(
        |p, x, y| {
            let (u, v) = p.centred(x, y);
            let r = 1024 + 6 * p.wob(x, y, 16, 311, 40);
            i64::from(u * u) * 1024 / (84 * 84) + i64::from(v * v) * 1024 / (76 * 76)
                <= i64::from(r)
        },
        WATER,
    );
    // Distributaries to the sea and the coasts (each with its mirror), and an inlet.
    p.stroke(&[(104, 80), (92, 56), (82, 32), (76, 0)], 4, WATER);
    p.stroke(&[(164, 84), (176, 60), (176, 32), (170, 0)], 4, WATER);
    p.stroke(&[(88, 116), (60, 120), (32, 128), (0, 124)], 4, WATER);
    p.stroke(&[(40, 80), (24, 88), (0, 88)], 2, WATER);
    // Marsh and mangrove along the channels and the lagoon's shore.
    p.paint(
        |p, x, y| {
            p.get(x, y) == GRASS
                && !p.near_start(x, y, 12)
                && p.noise(x, y, 9, 331) > 480
                && (-4..=4).any(|d| p.is_water(x + d, y) || p.is_water(x, y + d))
        },
        MARSH,
    );
    p.paint(
        |p, x, y| p.get(x, y) == MARSH && p.noise(x, y, 4, 332) > 640,
        MANGROVE,
    );
    // Jungle (thinner around the bases), palms on the shores.
    let starts = p.starts.clone();
    p.forests(15, 312, 560, move |x, y| {
        starts
            .iter()
            .all(|&(sx, sy)| (x - sx - 1).pow(2) + (y - sy - 1).pow(2) > 24 * 24)
    });
    p.forests(15, 312, 700, |_, _| true);
    p.paint(
        |p, x, y| {
            p.get(x, y) == GRASS
                && p.noise(x, y, 6, 313) > 650
                && (-3..=3).any(|d| p.is_water(x + d, y) || p.is_water(x, y + d))
        },
        PALMS,
    );
    // Islets in the lagoon.
    for (x, y, r, s) in [(112, 92, 6, 314), (144, 96, 4, 315), (120, 112, 4, 316)] {
        p.blob(x, y, r, 2, s, |p, x, y| {
            if p.noise(x, y, 5, 317) > 520 {
                FOREST
            } else {
                PALMS
            }
        });
    }
    // The river ports on the lagoon's shore, and fishing villages.
    p.city(
        |p, x, y| {
            let d = |cx: i32, cy: i32| (x - cx).pow(2) + (y - cy).pow(2) <= 14 * 14;
            let (mx, my) = p.mirror_of(144, 56);
            (d(144, 56) || d(mx, my)) && !p.is_water(x, y)
        },
        144,
        56,
        6,
        5,
        318,
    );
    for (x, y) in [(53, 61), (69, 153), (221, 81), (120, 40), (30, 110)] {
        p.village(x, y, 4, 319);
    }
    p.landmarks(20, 15, 333, |_, _, _| true);
    // Crossings: sand bars and timber bridges over the distributaries.
    p.stroke(&[(82, 48), (100, 48)], 5, SAND);
    p.stroke(&[(52, 112), (52, 134)], 5, SAND);
    p.stroke(&[(166, 16), (186, 16)], 5, SAND);
    p.road_x(19, 60, 100, 3);
    p.road_y(17, 108, 140, 3);
    p.road_x(69, 160, 196, 3);
    // Jungle tracks between the bases and the crossings.
    p.road_x(39, 24, 216, 2);
    p.road_y(33, 39, 140, 2);
    p.road_y(209, 39, 104, 2);
    // Ore.
    for (x, y, r, s) in [
        (113, 29, 4, 324),
        (149, 25, 5, 325),
        (65, 93, 5, 326),
        (201, 105, 4, 327),
        (60, 160, 4, 334),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 660);
    p.finish(2, 329);
}

/// Pacific Atoll, 256 x 256: open ocean around a ring of reef islands and its lagoon. Two
/// channels (north-east and south-west) cut the ring into two long arcs; a winding sand bar
/// runs from each arc to the island in the middle of the lagoon, the only way across by land.
/// The land is a tree, so the sea is one piece: a fleet can sail from any shore to any other,
/// through the channels and around the outside.
pub fn pacific_atoll(p: &mut Painter) {
    let (cx, cy) = (p.w / 2, p.h / 2);
    p.paint(|_, _, _| true, WATER);
    // The reef ring, broken by the two channels along the anti-diagonal.
    p.paint(
        |p, x, y| {
            let d2 = centre_d2(p, x, y);
            let inner = 66 + p.wob(x, y, 18, 400, 6) + p.wob(x, y, 7, 430, 2);
            let outer = 102 + p.wob(x, y, 22, 401, 6) + p.wob(x, y, 7, 431, 2);
            let (u, v) = p.centred(x, y);
            let channel = (u + v).abs() <= 30 + p.wob(x, y, 14, 402, 6);
            (inner * inner..=outer * outer).contains(&d2) && !channel
        },
        GRASS,
    );
    // The lagoon island.
    p.blob(cx, cy, 26, 6, 403, |_, _, _| GRASS);
    // Jungle, palm groves and rocky high ground on the arcs.
    p.forests(10, 404, 680, |_, _| true);
    p.paint(
        |p, x, y| p.get(x, y) == GRASS && p.noise(x, y, 6, 405) > 745,
        PALMS,
    );
    for (x, y, r, seed) in [(58, 100, 4, 406), (96, 54, 4, 407), (40, 168, 4, 408)] {
        p.blob(x, y, r, 2, seed, |_, _, _| ROCK);
    }
    // Islets: palms in the lagoon, rock and jungle out at sea.
    for (x, y, r, seed) in [(90, 116, 4, 409), (108, 88, 2, 410), (152, 100, 4, 411)] {
        p.blob(x, y, r, 2, seed, |_, _, _| PALMS);
    }
    for (x, y, r, seed) in [(18, 18, 8, 412), (224, 28, 6, 413), (16, 80, 4, 414)] {
        p.blob(x, y, r, 2, seed, |_, _, _| GRASS);
    }
    p.blob(80, 12, 4, 2, 415, |_, _, _| ROCK);
    // Fishing villages.
    for (x, y) in [(89, 61), (49, 113), (173, 49), (60, 196)] {
        p.village(x, y, 4, 416);
    }
    p.landmarks(18, 14, 432, |p, x, y| centre_d2(p, x, y) > 30 * 30);
    // A track along the middle of each arc keeps it open from end to end, and the sand bars
    // (each stroke with its mirror) reach a clearing in the middle of the lagoon island.
    p.paint(
        |p, x, y| !p.is_water(x, y) && (82 * 82..=86 * 86).contains(&centre_d2(p, x, y)),
        TRACK,
    );
    p.blob(cx, cy, 8, 2, 417, |_, _, _| GRASS);
    p.stroke(&[(132, 62), (122, 82), (130, 98), (126, 118)], 9, SAND);
    p.road_y(cx - 1, 44, 62, 2);
    // Ore: a field beside every base, fields at the ends of the arcs, and the lagoon island.
    for (x, y, r, s) in [
        (72, 192, 5, 428),
        (116, 120, 4, 427),
        (168, 58, 4, 426),
        (30, 150, 4, 423),
        (150, 30, 4, 425),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 680);
    p.finish(2, 429);
}

/// Iron Isthmus, 256 x 224: two continents in the open sea, joined only by a neck of land with a
/// port town on it. Each continent has capes, a lake, farmland and woods, and a sand spit out to
/// an ore island. The sea surrounds everything, so navies can sail around either continent and
/// shell any coast, while armies must fight through the isthmus.
pub fn iron_isthmus(p: &mut Painter) {
    let (cx, cy) = (p.w / 2, p.h / 2);
    p.paint(|_, _, _| true, WATER);
    // The north-western continent and its capes (the mirror is the south-eastern one).
    p.blob(68, 68, 50, 12, 440, |_, _, _| GRASS);
    p.blob(114, 38, 20, 6, 441, |_, _, _| GRASS);
    p.blob(34, 116, 20, 6, 442, |_, _, _| GRASS);
    p.blob(30, 30, 20, 6, 454, |_, _, _| GRASS);
    // The isthmus, and the sand spit to the ore island off the north-east cape.
    p.stroke(&[(104, 92), (cx, cy)], 14, GRASS);
    p.blob(180, 28, 12, 4, 443, |_, _, _| GRASS);
    p.stroke(&[(132, 34), (152, 26), (172, 28)], 4, SAND);
    // Skerries and wooded islets out at sea.
    for (x, y, r, seed) in [(220, 80, 4, 444), (140, 76, 2, 445), (12, 168, 4, 446)] {
        p.blob(x, y, r, 2, seed, |_, _, _| ROCK);
    }
    p.blob(208, 60, 6, 2, 447, |_, _, _| GRASS);
    // Inland: a lake in its marsh, a ridge that shelters the main base, farmland and woods.
    p.blob(76, 88, 12, 4, 455, |p, x, y| {
        if p.get(x, y) == GRASS {
            MARSH
        } else {
            p.get(x, y)
        }
    });
    p.blob(76, 88, 8, 4, 448, |_, _, _| WATER);
    p.stroke(&[(60, 60), (72, 54)], 2, ROCK);
    p.blob(92, 68, 4, 2, 449, |_, _, _| ROCK);
    farmland(p, 15, 450, 640);
    p.forests(18, 451, 625, |_, _| true);
    // The port town astride the isthmus, with open ground on both of its sides.
    p.city(
        |p, x, y| centre_d2(p, x, y) <= 10 * 10 && !p.is_water(x, y),
        cx,
        cy,
        6,
        5,
        452,
    );
    for (x, y) in [(81, 33), (29, 81), (185, 25), (60, 100)] {
        p.village(x, y, 4, 453);
    }
    p.landmarks(20, 15, 456, |p, x, y| centre_d2(p, x, y) > 40 * 40);
    // Roads from the bases to the isthmus.
    p.road_x(55, 53, 100, 2);
    p.road_y(101, 41, 88, 2);
    p.road_y(53, 55, 108, 2);
    p.road_x(37, 36, 53, 2);
    p.road_y(53, 37, 55, 2);
    // The neck stays open from continent to continent.
    p.stroke(&[(96, 84), (116, 102)], 4, GRASS);
    // Ore: fields at every base, the island, and contested fields at the neck.
    for (x, y, r, s) in [
        (182, 28, 5, 466),
        (97, 105, 5, 467),
        (117, 81, 4, 468),
        (70, 40, 4, 469),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 700);
    p.finish(2, 470);
}

fn polar(_: &Painter, _: i32, _: i32) -> Biome {
    ARCTIC
}

/// Arctic Floes, 240 x 240: islands of snow and rock in a polar sea, joined by shelves of sea
/// ice that armies cross and nobody can build on. The home islands lie in the north-west and
/// south-east, each joined to the middle island by a broad shelf: the only way across by land.
/// Each home island also has a northern and a western side island of its own at the end of a
/// narrower shelf. Open water runs between all of them and around the outside, past icebergs:
/// the land is a tree, the sea one piece.
pub fn arctic_floes(p: &mut Painter) {
    let (cx, cy) = (p.w / 2, p.h / 2);
    p.paint(|_, _, _| true, WATER);
    // Ice shelves: the broad one from home to the middle island, and those to the side islands.
    p.stroke(&[(68, 72), (84, 82), (100, 100)], 16, ICE);
    p.stroke(&[(80, 46), (100, 38), (118, 42)], 9, ICE);
    p.stroke(&[(46, 80), (38, 100), (42, 118)], 9, ICE);
    // The islands (each with its mirror): home, north, west, and the middle one.
    for (x, y, r, seed) in [
        (50, 50, 34, 480),
        (136, 42, 22, 481),
        (42, 136, 22, 482),
        (cx, cy, 26, 483),
    ] {
        p.blob(x, y, r, r / 4 + 1, seed, |_, _, _| SNOW);
    }
    // Rock: ridges on the islands, and icebergs in the sea.
    p.paint(
        |p, x, y| p.get(x, y) == SNOW && p.noise(x, y, 10, 484) > 735,
        SNOW_ROCK,
    );
    for (x, y, r, seed) in [
        (192, 20, 6, 485),
        (168, 80, 4, 486),
        (16, 200, 6, 487),
        (176, 60, 2, 488),
        (100, 14, 3, 500),
        (14, 96, 3, 501),
    ] {
        p.blob(x, y, r, 2, seed, |_, _, _| ICEBERG);
    }
    // Pine woods, frozen tarns, a weather station hamlet on each island.
    p.forests(15, 489, 640, |_, _| true);
    for (x, y) in [(65, 29), (149, 33), (33, 149), (109, 125)] {
        p.village(x, y, 3, 490);
    }
    p.landmarks(18, 14, 502, |p, x, y| centre_d2(p, x, y) > 20 * 20);
    // A cleared way along every shelf, from island to island, whatever grew or rose on it.
    let clear = |p: &Painter, x: i32, y: i32| {
        if [ICE, WATER].contains(&p.get(x, y)) {
            ICE
        } else {
            SNOW
        }
    };
    p.stroke_with(
        &[(56, 60), (68, 72), (84, 82), (100, 100), (114, 114)],
        4,
        clear,
    );
    p.stroke_with(
        &[(68, 46), (80, 46), (100, 38), (118, 42), (132, 40)],
        3,
        clear,
    );
    p.stroke_with(
        &[(46, 68), (46, 80), (38, 100), (42, 118), (40, 132)],
        3,
        clear,
    );
    // The ice road from each home base down its shelf to the middle island.
    p.road_x(61, 53, 80, 2);
    p.road_y(81, 61, 88, 2);
    p.road_x(89, 81, 104, 2);
    // Ore: fields at every base, at the heads of the shelves, and on the middle island.
    for (x, y, r, s) in [
        (105, 109, 5, 497),
        (62, 74, 4, 498),
        (125, 55, 4, 494),
        (55, 125, 4, 496),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 720);
    p.finish(1, 499);
}

/// The painted maps: id, biome function, recipe.
#[allow(clippy::type_complexity)]
pub const RECIPES: [(&str, fn(&Painter, i32, i32) -> Biome, fn(&mut Painter)); 18] = [
    ("liberty_bay", temperate, liberty_bay),
    ("frostline", arctic_biome, frostline),
    ("oasis_gulf", desert_biome, oasis_gulf),
    ("coral_archipelago", tropic, coral_archipelago),
    ("continental_divide", continental_biome, continental_divide),
    ("steel_harbor", temperate, steel_harbor),
    ("glacier_pass", glacier_biome, glacier_pass),
    ("red_canyon", desert, red_canyon),
    ("jungle_delta", tropic, jungle_delta),
    ("pacific_atoll", tropic, pacific_atoll),
    ("iron_isthmus", temperate, iron_isthmus),
    ("arctic_floes", polar, arctic_floes),
    (
        "ember_caldera",
        frontier_maps::caldera_biome,
        frontier_maps::ember_caldera,
    ),
    ("frozen_sound", polar, frontier_maps::frozen_sound),
    ("harvest_plains", temperate, frontier_maps::harvest_plains),
    ("black_water", tropic, frontier_maps::black_water),
    (
        "thousand_lakes",
        frontier_maps::lakeland_biome,
        frontier_maps::thousand_lakes,
    ),
    (
        "wheel_of_worlds",
        frontier_maps::wheel_biome,
        frontier_maps::wheel_of_worlds,
    ),
];
