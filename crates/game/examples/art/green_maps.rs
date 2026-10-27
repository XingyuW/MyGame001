//! Maps of green and lived-in country: an East African savanna, three rivers meeting at a city,
//! an old-growth forest, terraced rice hills and the suburbs of an American city.
//!
//! Seeds 5000 to 5999 are this file's: 5000s the savanna, 5100s the confluence, 5200s the
//! forest, 5300s the terraces, 5400s the suburbs.
use super::*;

/// Spots on a jittered grid over the upper half of the map, one to every `spacing` x `spacing`
/// cells, where `ok` holds, each with a roll for what to put there. Painted with the tools that
/// also paint the mirror, they cover the whole map.
fn spots(
    p: &Painter,
    spacing: i32,
    seed: u32,
    ok: impl Fn(&Painter, i32, i32) -> bool,
) -> Vec<(i32, i32, u32)> {
    let mut found = Vec::new();
    for gy in 0..=p.h / 2 / spacing {
        for gx in 0..=p.w / spacing {
            let roll = hash(gx, gy, seed);
            let x = gx * spacing + (roll % spacing as u32) as i32;
            let y = gy * spacing + ((roll >> 8) % spacing as u32) as i32;
            if y < p.h / 2 && p.inside(x, y) && ok(p, x, y) {
                found.push((x, y, roll >> 16));
            }
        }
    }
    found
}

/// A rectangle from (x0, y0) to (x1, y1) inclusive and its mirror, each cell painted with
/// `tile`.
fn rect(
    p: &mut Painter,
    (x0, y0): (i32, i32),
    (x1, y1): (i32, i32),
    tile: impl Fn(&Painter, i32, i32) -> u32,
) {
    let (mx, my) = p.mirror_of(x1, y1);
    for (ax, ay) in [(x0, y0), (mx, my)] {
        for y in ay..=ay + (y1 - y0) {
            for x in ax..=ax + (x1 - x0) {
                if p.inside(x, y) {
                    let t = tile(p, x, y);
                    p.set(x, y, t);
                }
            }
        }
    }
}

/// A ring of cells from `r0` to `r1` cells around (cx, cy) and its mirror, each cell painted
/// with `tile`.
fn ring(
    p: &mut Painter,
    (cx, cy): (i32, i32),
    r0: i32,
    r1: i32,
    tile: impl Fn(&Painter, i32, i32) -> u32,
) {
    let (mx, my) = p.mirror_of(cx, cy);
    for (ax, ay) in [(cx, cy), (mx, my)] {
        for y in ay - r1..=ay + r1 {
            for x in ax - r1..=ax + r1 {
                let d2 = (x - ax).pow(2) + (y - ay).pow(2);
                if p.inside(x, y) && (r0 * r0..=r1 * r1 + r1).contains(&d2) {
                    let t = tile(p, x, y);
                    p.set(x, y, t);
                }
            }
        }
    }
}

/// `tile` on the open ground of a cell, except on one cell in `keep` (hashed by `seed`, the same
/// cells on both halves); other ground stays as it is.
fn sprinkle(p: &Painter, x: i32, y: i32, seed: u32, keep: u32, tile: u32) -> u32 {
    let (kx, ky) = canonical(p, x, y);
    if p.is_plain(p.get(x, y)) && !hash(kx, ky, seed).is_multiple_of(keep) {
        tile
    } else {
        p.get(x, y)
    }
}

// ---------------------------------------------------------------------------------------------
// Savanna.

pub const SAVANNA: Layout = Layout {
    width: 256,
    height: 256,
    homes: &[(29, 29), (125, 22), (22, 125), (175, 22), (94, 94)],
};

/// The long grass of the savanna: mud huts, dirt tracks, and earth trampled bare (not sand) at
/// the water's edge.
const SAVANNA_GRASS: Biome = Biome {
    plain: GRASS,
    ore: ORE,
    cliff: ROCK,
    forest: FOREST,
    house: ADOBE,
    road: TRACK,
    shore: DIRT,
    block: CITY,
    tower: TOWER,
};

/// Its dry rises of bare earth and burnt-off grass.
const SAVANNA_DRY: Biome = Biome {
    plain: DIRT,
    ..SAVANNA_GRASS
};

pub fn savanna_biome(p: &Painter, x: i32, y: i32) -> Biome {
    if p.noise(x, y, 30, 5000) > 545 {
        SAVANNA_DRY
    } else {
        SAVANNA_GRASS
    }
}

/// A boma at (x, y) and its mirror: a ring of mud huts around a cattle kraal of bare earth.
fn boma(p: &mut Painter, x: i32, y: i32, seed: u32) {
    p.patch(x, y, 6, 1, seed, DIRT);
    ring(p, (x, y), 3, 4, |p, x, y| sprinkle(p, x, y, seed, 3, ADOBE));
}

/// Savanna, 256 x 256: the Serengeti in the dry season. Rolling grass with wide stretches of
/// bare earth, lone acacias and a few clumps of them, granite kopjes standing out of the plain,
/// and waterholes ringed by reeds and trampled mud. A seasonal river crosses the map from corner
/// to corner, pools and sandbars in a band of gallery forest, forded where the dust tracks meet
/// it; bomas and a market village lie along the tracks. Open, fast tank country.
pub fn savanna(p: &mut Painter) {
    let w = p.w;
    let n = 2 * w - 1;
    // Across the river (cells along x, negative on the north-west bank): it runs from the
    // north-east corner to the south-west one and is its own mirror.
    let bend = move |p: &Painter, along: i32| p.meander(along + w - 1, n, 64, 5002, 26);
    let off = move |p: &Painter, x: i32, y: i32| x + y - (w - 1) - bend(p, x - y);
    // A point on the river, `along` cells (x - y) from the centre.
    let river_at = move |p: &Painter, along: i32| {
        let sum = w - 1 + bend(p, along);
        ((sum + along) / 2, (sum - along) / 2)
    };
    // The river: pools and runs between dry sandbars, sandy banks, and the gallery forest that
    // thins out away from the water.
    p.fill(
        |p, x, y| off(p, x, y).abs() <= 17,
        |p, x, y| {
            let o = off(p, x, y).abs();
            if o <= 3 || (o <= 6 && p.noise(x, y, 20, 5006) > 690) {
                if p.noise(x, y, 14, 5003) > 600 {
                    SAND
                } else {
                    WATER
                }
            } else if o <= 5 {
                if p.noise(x, y, 5, 5004) > 450 {
                    SAND
                } else {
                    DIRT
                }
            } else if p.noise(x, y, 4, 5005) > i64::from(290 + 18 * (o - 6)) {
                FOREST
            } else {
                p.get(x, y)
            }
        },
    );
    // Waterholes, each in a ring of reeds and of mud trampled bare by the herds.
    for (x, y, r, s) in [
        (66, 62, 3, 5007),
        (160, 50, 2, 5008),
        (48, 166, 2, 5009),
        (84, 132, 2, 5010),
        (210, 14, 2, 5011),
        (126, 70, 2, 5012),
    ] {
        p.patch(x, y, r + 8, 2, s, DIRT);
        p.patch(x, y, r + 3, 2, s + 1, MARSH);
        p.blob(x, y, r, 1, s + 2, |_, _, _| WATER);
    }
    // Kopjes: granite outcrops of a few boulders each, standing out of the grass.
    let kopjes = spots(p, 26, 5013, |p, x, y| {
        !p.near_start(x, y, 20) && off(p, x, y).abs() > 24
    });
    for (x, y, roll) in kopjes {
        if roll % 5 < 2 {
            continue;
        }
        for b in 0..3 + (roll % 3) as i32 {
            let dx = (hash(x, b, 5014) % 13) as i32 - 6;
            let dy = (hash(y, b, 5015) % 13) as i32 - 6;
            let r = 1 + (hash(x + b, y, 5016) % 3) as i32;
            p.patch(x + dx, y + dy, r, 1, 5017 + b as u32, ROCK);
        }
    }
    // Acacias: lone trees dotted over the plain, and here and there a clump.
    p.paint(
        |p, x, y| {
            let (kx, ky) = canonical(p, x, y);
            p.is_plain(p.get(x, y))
                && !p.near_start(x, y, 14)
                && (hash(kx, ky, 5020).is_multiple_of(53)
                    || (p.noise(x, y, 4, 5021) > 800 && p.noise(x, y, 36, 5022) > 500))
        },
        FOREST,
    );
    // Bomas along the tracks, and a market village by the central ford.
    for (x, y, s) in [
        (80, 30, 5023),
        (30, 80, 5024),
        (152, 80, 5025),
        (60, 112, 5026),
        (204, 30, 5027),
    ] {
        boma(p, x, y, s);
    }
    let (rx, ry) = river_at(p, 0);
    p.patch(rx + 2, ry - 25, 2, 1, 5028, PLAZA);
    p.village(rx + 2, ry - 25, 5, 5029);
    // Fords where the tracks meet the river: its sandbars, cleared of trees.
    let crossings = [river_at(p, 0), river_at(p, 46), river_at(p, 150)];
    let ford = |p: &Painter, x: i32, y: i32| match p.get(x, y) {
        WATER => SAND,
        FOREST => p.biome(x, y).plain,
        t => t,
    };
    for (x, y) in crossings {
        p.stroke_with(&[(x - 9, y - 9), (x + 9, y + 9)], 6, ford);
    }
    // The dust tracks between the bases, the bomas and the fords.
    let [c0, c1, c2] = crossings;
    let track = |p: &Painter, x: i32, y: i32| match p.get(x, y) {
        WATER | SAND => p.get(x, y),
        _ => TRACK,
    };
    let tracks: [&[(i32, i32)]; 8] = [
        &[(30, 30), (46, 44), (58, 44), (80, 30), (104, 34), (126, 23)],
        &[(30, 30), (44, 46), (44, 58), (30, 80), (36, 104), (23, 126)],
        &[(30, 30), (54, 60), (74, 66), (80, 80), (95, 95), c0],
        &[(126, 23), (140, 36), (160, 30), (176, 23)],
        &[(126, 23), (120, 46), (128, 70), (110, 86), (95, 95)],
        &[(23, 126), (46, 120), (60, 112), (84, 108), (95, 95)],
        &[(126, 23), (138, 56), (152, 80), c1, (154, 154)],
        &[(176, 23), (204, 30), c2, (222, 90), (232, 129)],
    ];
    for path in tracks {
        p.stroke_with(path, 1, track);
    }
    // Ore: fields on both banks at every ford, which the bases contest, and fields on the
    // plain between neighbouring bases.
    for (k, &(x, y)) in crossings.iter().enumerate() {
        p.ore(x - 16, y - 12, 5, 5040 + k as u32);
    }
    for (x, y, r, s) in [
        (78, 46, 4, 5044),
        (46, 78, 4, 5045),
        (152, 24, 4, 5046),
        (64, 92, 4, 5047),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 5090);
    p.finish(1, 5091);
}

// ---------------------------------------------------------------------------------------------
// River Confluence.

pub const RIVER_CONFLUENCE: Layout = Layout {
    width: 256,
    height: 256,
    homes: &[(29, 29), (130, 22), (22, 128), (62, 98), (174, 18)],
};

pub fn river_confluence_biome(_: &Painter, _: i32, _: i32) -> Biome {
    TEMPERATE
}

/// River Confluence, 256 x 256: three rivers meet at a city, as at Koblenz. The great river
/// flows from the north-east corner to the south-west one; a winding tributary comes down from
/// the north and joins it in the city, and another, from the south, joins it just across the
/// water. Floodplain meadows and marshes line the valleys, vineyards their slopes, and
/// broadleaf forest with farms in its clearings covers the uplands between them, where the
/// bases lie. Bridges cross everywhere; the tributaries have fords, the great river a gravel
/// bar.
pub fn river_confluence(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    let (cx, cy) = (w / 2, h / 2);
    let n = 2 * w - 1;
    // Across the great river, cells along x: it is its own mirror.
    let bend = move |p: &Painter, along: i32| p.meander(along + w - 1, n, 90, 5102, 22);
    let rhine = move |p: &Painter, x: i32, y: i32| (x + y - (w - 1) - bend(p, x - y)).abs();
    let rhine_at = move |p: &Painter, along: i32| {
        let sum = w - 1 + bend(p, along);
        ((sum + along) / 2, (sum - along) / 2)
    };
    // The northern tributary's middle at row y, winding down from the north edge to the
    // confluence; the southern one is its mirror.
    let xm = |y: i32| {
        let taper = i64::from((126 - y).clamp(0, 30));
        56 + y * 67 / 126 + ((fbm(0, y, 40, 5103) - 512) * 44 * taper / (512 * 30)) as i32
    };
    let tributary = move |x: i32, y: i32| {
        let d = |x: i32, y: i32| {
            if (0..=126).contains(&y) {
                (x - xm(y)).abs()
            } else {
                99
            }
        };
        d(x, y).min(d(w - 1 - x, h - 1 - y))
    };
    // About how many cells a cell lies from the nearest river.
    let valley = move |p: &Painter, x: i32, y: i32| (rhine(p, x, y) * 7 / 10).min(tributary(x, y));
    p.paint(|p, x, y| rhine(p, x, y) <= 7, WATER);
    let moselle: Vec<(i32, i32)> = (0..=63).map(|k| (xm(2 * k), 2 * k)).collect();
    p.stroke(&moselle, 3, WATER);
    // Floodplain marsh on the valley floors, vineyards in rows on the slopes above them.
    p.paint(
        |p, x, y| {
            !p.is_water(x, y)
                && valley(p, x, y) <= 14
                && p.noise(x, y, 9, 5106) > 560
                && centre_d2(p, x, y) > 40 * 40
        },
        MARSH,
    );
    p.fill(
        |p, x, y| {
            p.get(x, y) == GRASS
                && (9..=22).contains(&valley(p, x, y))
                && p.noise(x, y, 18, 5107) > 610
        },
        |p, x, y| {
            let (kx, ky) = canonical(p, x, y);
            if ky % 3 == 0 && kx % 9 != 0 {
                ORCHARD
            } else {
                DIRT
            }
        },
    );
    // Broadleaf forest on the uplands, thicker the farther from the rivers, and farms in its
    // clearings.
    p.fill(
        |p, x, y| {
            p.get(x, y) == GRASS
                && p.noise(x, y, 20, 5104) > i64::from(660 - 3 * valley(p, x, y).min(40))
        },
        |_, _, _| FOREST,
    );
    farmland(p, 15, 5105, 620);
    // Clearings for the bases.
    let homes: Vec<(i32, i32)> = p.starts.iter().copied().step_by(2).collect();
    for (k, &(sx, sy)) in homes.iter().enumerate() {
        p.blob(sx + 1, sy + 1, 20, 4, 5120 + k as u32, |p, x, y| {
            if p.is_water(x, y) { WATER } else { GRASS }
        });
    }
    // The city at the confluence, a fortress on the crag across the water, and river towns.
    p.city(
        |p, x, y| centre_d2(p, x, y) <= (30 + p.wob(x, y, 10, 5108, 4)).pow(2),
        cx,
        cy,
        13,
        6,
        5109,
    );
    p.blob(160, 104, 6, 2, 5111, |p, x, y| {
        if p.is_water(x, y) { WATER } else { ROCK }
    });
    p.blob(160, 104, 3, 1, 5112, |_, _, _| CITY);
    p.blob(160, 104, 1, 1, 5113, |_, _, _| PLAZA);
    for (x, y) in [(xm(22) - 12, 22), (xm(64) + 12, 64), (xm(104) - 12, 104)] {
        p.village(x, y, 4, 5114);
    }
    for (along, side) in [(70, 12), (150, -12), (200, 12)] {
        let (x, y) = rhine_at(p, along);
        p.village(x + side, y + side, 4, 5115);
    }
    // Roads: avenues through the city and its bridges, the upland roads between the bases and
    // their bridges.
    p.road_x(cy - 1, 16, w - 17, 2);
    p.road_y(cx - 1, 16, h - 17, 2);
    p.road_x(30, 30, 174, 2);
    p.road_y(30, 30, 128, 2);
    p.road_l((63, 99), (140, 24), 2);
    p.road_l((150, 64), (226, 126), 2);
    // Fords on the tributaries, and a gravel bar across the great river.
    let bar = |p: &Painter, x: i32, y: i32| match p.get(x, y) {
        WATER | MARSH => SAND,
        FOREST => GRASS,
        t => t,
    };
    for y in [10, 76] {
        let x = xm(y);
        p.stroke_with(&[(x - 10, y - 2), (x + 10, y + 2)], 5, bar);
    }
    let (gx, gy) = rhine_at(p, 150);
    p.stroke_with(&[(gx - 10, gy - 10), (gx + 10, gy + 10)], 6, bar);
    // Ore: fields on the valley floors by the crossings, which the bases contest, and on the
    // uplands.
    let (ax, ay) = rhine_at(p, 70);
    for (x, y, r, s) in [
        (xm(76) + 16, 70, 5, 5130),
        (gx - 18, gy - 10, 5, 5131),
        (ax - 16, ay - 16, 5, 5132),
        (96, 40, 4, 5133),
        (36, 70, 4, 5134),
        (204, 96, 4, 5135),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 5190);
    p.finish(1, 5191);
}

// ---------------------------------------------------------------------------------------------
// Old Growth.

pub const OLD_GROWTH: Layout = Layout {
    width: 256,
    height: 256,
    homes: &[(29, 29), (118, 24), (24, 118), (196, 26), (90, 90)],
};

pub fn old_growth_biome(_: &Painter, _: i32, _: i32) -> Biome {
    TEMPERATE
}

/// Old Growth, 256 x 256: an ancient forest, as at Białowieża. Trees cover the map, divided
/// into great compartments by the straight rides of the old forest survey, four each way; armies
/// move along the rides, river meadows and clearings, with many ways between any two bases. A
/// lake lies at the centre in a meadow ring with sawmill villages, rivers flow from it to the
/// west and east edges, and inside the compartments are bogs with alder carr, glades,
/// clear-cuts and the burnt ground of an old fire. The bases stand in large clearings where
/// rides cross.
pub fn old_growth(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    p.paint(|_, _, _| true, FOREST);
    // Outcrops and bogs among the trees; enclosed by them, a bog grows into alder carr.
    p.paint(|p, x, y| p.noise(x, y, 9, 5206) > 830, ROCK);
    p.paint(|p, x, y| p.noise(x, y, 16, 5202) > 720, MARSH);
    // The meadow round the lake, the lake, and the rivers from it in their meadows.
    p.paint(
        |p, x, y| centre_d2(p, x, y) <= (40 + p.wob(x, y, 10, 5204, 5)).pow(2),
        GRASS,
    );
    p.paint(
        |p, x, y| centre_d2(p, x, y) <= (22 + p.wob(x, y, 14, 5203, 6)).pow(2),
        WATER,
    );
    let river = [
        (106, 132),
        (84, 140),
        (62, 134),
        (40, 148),
        (18, 150),
        (-2, 144),
    ];
    let meadow = |p: &Painter, x: i32, y: i32| {
        if p.is_water(x, y) {
            WATER
        } else if p.noise(x, y, 8, 5207) > 620 {
            MARSH
        } else {
            GRASS
        }
    };
    p.stroke_with(&river, 7, meadow);
    p.stroke(&river, 2, WATER);
    // The rides: straight cuttings across the whole forest, the outer ones wider.
    let cut = |p: &Painter, x: i32, y: i32| match p.get(x, y) {
        WATER => WATER,
        _ if p.noise(x, y, 6, 5205) > 600 => DIRT,
        _ => GRASS,
    };
    for (a, r) in [(30, 6), (92, 5)] {
        p.stroke_with(&[(a, 0), (a, h - 1)], r, cut);
        p.stroke_with(&[(0, a), (w - 1, a)], r, cut);
        // A broad ford where the ride crosses the river.
        p.stroke_with(&[(a, 120), (a, 165)], r, |p, x, y| {
            if p.is_water(x, y) { SAND } else { p.get(x, y) }
        });
    }
    // Two old roads that wind through the compartments, older than the rides.
    let old_roads = [
        [(92, 30), (108, 44), (116, 62), (126, 90)],
        [(163, 92), (186, 80), (200, 58), (225, 30)],
    ];
    for path in &old_roads {
        p.stroke_with(path, 4, cut);
    }
    // Clearings: the bases', and glades and meadows inside the compartments.
    let homes: Vec<(i32, i32)> = p.starts.iter().copied().step_by(2).collect();
    for (k, &(sx, sy)) in homes.iter().enumerate() {
        p.blob(sx + 1, sy + 1, 22, 5, 5210 + k as u32, |p, x, y| {
            if p.is_water(x, y) { WATER } else { GRASS }
        });
    }
    for (x, y, r, s) in [
        (60, 64, 10, 5216),
        (128, 60, 12, 5217),
        (196, 124, 11, 5218),
        (62, 196, 10, 5219),
        (194, 62, 8, 5220),
    ] {
        p.blob(x, y, r, 4, s, meadow);
    }
    // Clear-cuts along the rides, and the burnt ground of an old fire with its dead trees.
    for (a, b) in [
        ((38, 37), (62, 50)),
        ((140, 37), (156, 56)),
        ((170, 98), (186, 122)),
        ((37, 170), (50, 190)),
    ] {
        rect(p, a, b, |p, x, y| match p.get(x, y) {
            FOREST | MARSH => DIRT,
            t => t,
        });
    }
    p.blob(204, 196, 13, 4, 5230, |p, x, y| match p.get(x, y) {
        FOREST | GRASS | DIRT | MARSH => ASH,
        t => t,
    });
    p.paint(
        |p, x, y| {
            let (kx, ky) = canonical(p, x, y);
            p.get(x, y) == ASH && hash(kx, ky, 5232).is_multiple_of(5)
        },
        ASH_WOODS,
    );
    // Alder carr in the bogs.
    p.paint(
        |p, x, y| p.get(x, y) == MARSH && p.noise(x, y, 4, 5233) > 680,
        MANGROVE,
    );
    // Sawmill villages where rides cross and on the lake shore, each with its timber yard.
    for (x, y) in [(163, 30), (30, 163), (142, 94), (100, 104)] {
        p.patch(x, y, 3, 1, 5234, PLAZA);
        p.village(x, y, 6, 5235);
    }
    // The roads down the rides, the shore road round the lake, and their bridges.
    let road = |p: &Painter, x: i32, y: i32| {
        if p.is_water(x, y) { BRIDGE } else { TRACK }
    };
    for a in [30, 92] {
        p.stroke_with(&[(a, 0), (a, h - 1)], 1, road);
        p.stroke_with(&[(0, a), (w - 1, a)], 1, road);
    }
    for path in &old_roads {
        p.stroke_with(path, 1, road);
    }
    p.fill(
        |p, x, y| (33 * 33..=35 * 35).contains(&centre_d2(p, x, y)),
        road,
    );
    // Ore: on the lake shore and in the meadows, which the bases contest, and in the
    // clear-cuts.
    for (x, y, r, s) in [
        (100, 142, 5, 5240),
        (128, 62, 5, 5241),
        (196, 126, 5, 5242),
        (60, 62, 4, 5243),
        (148, 46, 4, 5244),
        (178, 110, 4, 5245),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 5290);
    p.finish(1, 5291);
}

// ---------------------------------------------------------------------------------------------
// Rice Terraces.

pub const RICE_TERRACES: Layout = Layout {
    width: 224,
    height: 224,
    homes: &[(29, 29), (110, 22), (190, 40), (72, 78)],
};

pub fn rice_terraces_biome(_: &Painter, _: i32, _: i32) -> Biome {
    TROPIC
}

/// Rice Terraces, 224 x 224: terraced hill country, as at Banaue or in Yunnan. A river winds
/// from west to east through a valley of fields, palms and villages; above it the hillsides are
/// stepped into terraces that follow the contours, rice paddies flooded or green, held up by low
/// walls of stone and earth with many gaps. Bamboo and forest cover the steepest ridges, and
/// streams run down through the terraces. Bases lie on the valley floor and on broad saddles
/// in the hills.
pub fn rice_terraces(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    // The river's line (doubled rows from the centre), its own mirror, and how many cells a
    // cell lies from it.
    let line = move |p: &Painter, x: i32| {
        let (u, _) = p.centred(x, 0);
        u * 2 / 5 + 2 * p.meander(x, w, 44, 5302, 14)
    };
    let across = move |p: &Painter, x: i32, y: i32| {
        let (_, v) = p.centred(x, y);
        (v - line(p, x)).abs() / 2
    };
    let river_y = move |p: &Painter, x: i32| (line(p, x) + h - 1) / 2;
    // The lie of the land: rising from the river, with hills and hollows.
    let height = move |p: &Painter, x: i32, y: i32| {
        across(p, x, y) * 10 + (p.noise(x, y, 36, 5303) as i32 - 512) * 3 / 5
    };
    // Terraces up the slopes: bands of paddy, flooded or green, stepped by walls with gaps;
    // bamboo groves on the spurs and forest on the ridges.
    p.fill(
        |p, x, y| height(p, x, y) >= 170,
        |p, x, y| {
            let hh = height(p, x, y);
            let (kx, ky) = canonical(p, x, y);
            if hh >= 880 {
                if p.noise(x, y, 8, 5307) > 760 {
                    ROCK
                } else {
                    FOREST
                }
            } else if p.noise(x, y, 12, 5306) > 720 {
                FOREST
            } else if (hh - 170) % 40 < 7 && !hash(kx / 3, ky / 3, 5304).is_multiple_of(4) {
                ROCK
            } else if ((hh - 170) / 40 + i32::from(p.noise(x, y, 28, 5305) > 512)) % 2 == 0 {
                MARSH
            } else {
                CROPS
            }
        },
    );
    // The valley floor: fields, the river, and palms along its banks.
    farmland(p, 12, 5309, 540);
    p.paint(|p, x, y| across(p, x, y) <= 2, WATER);
    p.paint(
        |p, x, y| {
            [GRASS, DIRT, CROPS].contains(&p.get(x, y))
                && across(p, x, y) <= 7
                && p.noise(x, y, 6, 5310) > 600
        },
        PALMS,
    );
    // Streams down through the terraces.
    let streams = [vec![
        (152, -2),
        (146, 16),
        (154, 36),
        (147, 56),
        (156, 76),
        (150, 96),
        (152, river_y(p, 152)),
    ]];
    for s in &streams {
        p.stroke(s, 1, WATER);
        // Fords over stepping stones.
        for &(x, y) in s.iter().skip(1).step_by(2).take(3) {
            p.stroke_with(&[(x - 5, y), (x + 5, y)], 4, |p, x, y| {
                if p.is_water(x, y) { SAND } else { p.get(x, y) }
            });
        }
    }
    // Level ground for the bases: the valley floor, and saddles in the hills.
    let homes: Vec<(i32, i32)> = p.starts.iter().copied().step_by(2).collect();
    for (k, &(sx, sy)) in homes.iter().enumerate() {
        p.blob(sx + 1, sy + 1, 20, 6, 5320 + k as u32, |p, x, y| {
            if p.is_water(x, y) {
                WATER
            } else if p.noise(x, y, 7, 5325) > 640 {
                CROPS
            } else {
                GRASS
            }
        });
    }
    // Villages of the valley and of the terraces.
    for (x, y) in [(60, 64), (150, 88), (96, 60), (34, 96), (196, 120)] {
        p.village(x, y, 4, 5330);
    }
    // Roads: along the valley on both banks, up to the bases on the hills, and the bridges.
    let open = |p: &Painter, x: i32, y: i32| match p.get(x, y) {
        FOREST | ROCK => GRASS,
        t => t,
    };
    let valley_road: Vec<(i32, i32)> = (0..=14)
        .map(|k| {
            let x = k * 16;
            (x, river_y(p, x) - 9)
        })
        .collect();
    let hill_roads = [
        vec![
            (30, 30),
            (46, 40),
            (36, 52),
            (54, 60),
            (52, river_y(p, 52) - 9),
        ],
        vec![
            (111, 23),
            (98, 36),
            (112, 48),
            (98, 62),
            (100, river_y(p, 100) - 9),
        ],
        vec![
            (191, 41),
            (176, 56),
            (190, 72),
            (172, 86),
            (182, 100),
            (180, river_y(p, 180) - 9),
        ],
        vec![(60, river_y(p, 60) - 9), (60, river_y(p, 60) + 9)],
        vec![(112, river_y(p, 112) - 9), (112, river_y(p, 112) + 9)],
    ];
    for path in &hill_roads {
        p.stroke_with(path, 4, open);
    }
    p.stroke_with(&valley_road, 1, |p, x, y| p.road_tile(x, y));
    for path in &hill_roads {
        p.stroke_with(path, 1, |p, x, y| p.road_tile(x, y));
    }
    // Ore: on the valley floor, which the bases contest, and on the terraces.
    for (x, y, r, s) in [
        (112, river_y(p, 112) - 16, 5, 5340),
        (40, river_y(p, 40) + 14, 5, 5341),
        (150, 40, 4, 5342),
        (70, 30, 4, 5343),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 5390);
    p.finish(1, 5391);
}

// ---------------------------------------------------------------------------------------------
// Suburbia.

pub const SUBURBIA: Layout = Layout {
    width: 256,
    height: 256,
    homes: &[(28, 28), (109, 16), (180, 21), (231, 73), (237, 144)],
};

pub fn suburbia_biome(_: &Painter, _: i32, _: i32) -> Biome {
    TEMPERATE
}

/// Suburbia, 256 x 256: the sprawl of an American city. Downtown towers stand at the centre;
/// around them ring on ring of suburbs, curving streets of houses on their lots with lawns
/// behind, broken by parks, a golf course with bunkers and ponds, a shopping mall in its car
/// park and the brownfield lots of old industry. Freeways cross the map and meet a beltway at
/// cloverleaf interchanges; beyond it farms and woods, where the bases lie.
pub fn suburbia(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    let (cx, cy) = (w / 2, h / 2);
    // The countryside: farms and woods.
    farmland(p, 18, 5402, 560);
    p.forests(16, 5403, 620, |_, _| true);
    // The suburbs: streets that curve (a grid bent by a displacement odd about the centre, so
    // it is its own mirror), lawns between them.
    let odd = move |x: i32, y: i32, seed: u32| {
        ((fbm(x, y, 56, seed) - fbm(w - 1 - x, h - 1 - y, 56, seed)) * 6 / 512) as i32
    };
    let suburb = move |p: &Painter, x: i32, y: i32| {
        centre_d2(p, x, y) <= (78 + p.wob(x, y, 20, 5404, 8)).pow(2)
    };
    p.fill(suburb, |p, x, y| {
        let (u, v) = p.centred(x, y);
        let (su, sv) = (u + 2 * odd(x, y, 5405), v + 2 * odd(x, y, 5406));
        // A street three cells wide every `period / 2` cells (an odd number, so that the
        // street is its own mirror).
        let line = |c: i32, period: i32| (c.rem_euclid(period) - period / 2).abs() <= 2;
        let d2 = centre_d2(p, x, y);
        let street = if d2 < 54 * 54 {
            // The older suburbs: a grid.
            line(su, 30) || line(sv, 22)
        } else {
            // The newer: crescents round the city, crossed by the through roads.
            let r = p.wob(x, y, 26, 5408, 6);
            [55, 66, 77]
                .iter()
                .any(|&a| ((a + r).pow(2)..(a + r + 3).pow(2)).contains(&d2))
                || line(su, 62)
                || line(sv, 46)
        };
        if street { STREET } else { GRASS }
    });
    // Houses on the lots facing the streets, a tree here and there in the gardens.
    p.fill(
        |p, x, y| suburb(p, x, y) && p.get(x, y) == GRASS,
        |p, x, y| {
            let (kx, ky) = canonical(p, x, y);
            let front = (-2..=2).any(|dy| {
                (-2..=2).any(|dx| p.inside(x + dx, y + dy) && p.get(x + dx, y + dy) == STREET)
            });
            let roll = hash(kx, ky, 5407) % 12;
            if front && roll < 9 {
                COTTAGE
            } else if !front && roll == 0 {
                FOREST
            } else {
                GRASS
            }
        },
    );
    // Downtown.
    p.city(
        |p, x, y| centre_d2(p, x, y) <= (28 + p.wob(x, y, 10, 5410, 3)).pow(2),
        cx,
        cy,
        17,
        6,
        5411,
    );
    // The golf course: fairways and greens, tree lines, sand bunkers, ponds and a clubhouse.
    p.blob(78, 66, 20, 4, 5412, |p, x, y| {
        if p.noise(x, y, 6, 5413) > 650 {
            FOREST
        } else if p.noise(x, y, 3, 5414) > 820 {
            SAND
        } else {
            GRASS
        }
    });
    p.blob(70, 74, 3, 1, 5415, |_, _, _| WATER);
    p.blob(88, 58, 2, 1, 5416, |_, _, _| WATER);
    rect(p, (62, 50), (66, 53), |_, _, _| PLAZA);
    rect(p, (63, 51), (64, 52), |_, _, _| COTTAGE);
    // The mall in its car park.
    rect(p, (164, 74), (192, 92), |_, _, _| PLAZA);
    rect(p, (170, 80), (186, 85), |_, _, _| CITY);
    rect(p, (176, 80), (179, 82), |_, _, _| TOWER);
    // Parks with ponds, and brownfield lots of old industry.
    for (x, y, s) in [(128, 70, 5420), (98, 112, 5421), (60, 132, 5422)] {
        p.blob(x, y, 8, 3, s, |p, x, y| match p.get(x, y) {
            COTTAGE | STREET | DIRT | CROPS => GRASS,
            t => t,
        });
        p.blob(x + 2, y + 1, 2, 1, s + 1, |_, _, _| WATER);
        p.patch(x - 4, y - 3, 2, 1, s + 2, FOREST);
    }
    for (x, y, s) in [(160, 44, 5425), (44, 98, 5426)] {
        p.blob(
            x,
            y,
            9,
            3,
            s,
            |p, x, y| {
                if p.is_water(x, y) { WATER } else { DIRT }
            },
        );
    }
    // Freeways across the map, the beltway, and the cloverleaf interchanges where they meet.
    p.road_x(cy - 2, 0, w - 1, 4);
    p.road_y(cx - 2, 0, h - 1, 4);
    p.fill(
        |p, x, y| (88 * 88..=92 * 92).contains(&centre_d2(p, x, y)),
        |p, x, y| p.road_tile(x, y),
    );
    for (x, y) in [(cx, cy - 90), (cx - 90, cy)] {
        for (dx, dy) in [(-8, -8), (8, -8), (-8, 8), (8, 8)] {
            ring(p, (x + dx, y + dy), 5, 6, |p, x, y| p.road_tile(x, y));
        }
    }
    // Ore: in the brownfields, which the bases contest, and in the farmland between the bases.
    for (x, y, r, s) in [
        (160, 44, 5, 5440),
        (44, 98, 5, 5441),
        (70, 18, 4, 5442),
        (146, 14, 4, 5443),
        (214, 40, 4, 5444),
        (240, 108, 4, 5445),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 5490);
    p.finish(1, 5491);
}
