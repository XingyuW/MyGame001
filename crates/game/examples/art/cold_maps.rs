//! Maps of the cold north: a fjord coast, the open tundra, and an ice shelf split by rifts.
//!
//! * `fjord_coast` (256 x 256): long branching fjords between steep walls, green shores and
//!   snowy benches, fishing towns at the heads and the high fjell between the two coasts.
//! * `tundra_steppe` (256 x 256): a flat snow plain dotted with frozen lakes, crossed by eskers,
//!   frozen rivers in their taiga and one straight road to a mining town.
//! * `ice_shelf_rift` (240 x 240): a sheet of shelf ice, nunataks for the bases, and rifts of open
//!   water crossed by broad ice bridges.
//!
//! Seeds 4000 to 4999 are this file's.
use super::*;

/// Squared distance from (x, y) to the segment from `a` to `b`.
fn segment_d2(x: i32, y: i32, (ax, ay): (i32, i32), (bx, by): (i32, i32)) -> i32 {
    let (dx, dy) = (bx - ax, by - ay);
    let len2 = (dx * dx + dy * dy).max(1);
    let t = ((x - ax) * dx + (y - ay) * dy).clamp(0, len2);
    let (px, py) = (ax + dx * t / len2, ay + dy * t / len2);
    (x - px).pow(2) + (y - py).pow(2)
}

/// Distance in cells from (x, y) to a line of points or to its point mirror, whichever is
/// nearer: the same for a cell and its mirror.
fn line_distance(p: &Painter, x: i32, y: i32, line: &[(i32, i32)]) -> i32 {
    let (mx, my) = p.mirror_of(x, y);
    line.windows(2)
        .map(|s| segment_d2(x, y, s[0], s[1]).min(segment_d2(mx, my, s[0], s[1])))
        .min()
        .unwrap_or(i32::MAX)
        .isqrt()
}

/// How far (x, y) lies from the water of the nearest of `lines` (each a line of points and its
/// half-width): zero or less in the water.
fn gap(p: &Painter, x: i32, y: i32, lines: &[(&[(i32, i32)], i32)]) -> i32 {
    lines
        .iter()
        .map(|&(line, half)| line_distance(p, x, y, line) - half)
        .min()
        .unwrap_or(i32::MAX)
}

/// Point-symmetric noise on stretched coordinates (`a`, `b`) = `m` (x, y), so that its features
/// are drawn out along one direction: oriented lakes, eskers.
fn stretched(p: &Painter, x: i32, y: i32, m: (i32, i32, i32, i32), scale: i32, seed: u32) -> i64 {
    let at = |x: i32, y: i32| fbm(m.0 * x + m.1 * y, m.2 * x + m.3 * y, scale, seed);
    let n = (at(x, y) + at(p.w - 1 - x, p.h - 1 - y)) / 2;
    (512 + (n - 512) * 8 / 5).clamp(0, 1024)
}

/// A road three cells wide through `points` (and its mirror), bridging water on the way.
fn road(p: &mut Painter, points: &[(i32, i32)]) {
    p.stroke_with(points, 1, |p, x, y| p.road_tile(x, y));
}

// ---------------------------------------------------------------------------------------------
// Fjord Coast.

pub const FJORD_COAST: Layout = Layout {
    width: 256,
    height: 256,
    homes: &[(40, 16), (52, 66), (44, 118), (54, 172), (42, 226)],
};

/// The fjords of the western coast, each a line from its mouth to its head (or an arm from where
/// it branches) and its half-width; the fjords of the eastern coast are their mirrors. The heads
/// stop short of x = 100, so that the fjell between the two coasts stays some 60 cells wide:
/// with the heads at x = 110 (and their towns) it narrowed to 20 between a head and the mirror
/// of another, and the waves of two easy computer players met there head-on until the end of a
/// match (docs/DECISIONS.md, "Twenty-one more maps").
const FJORDS: [(&[(i32, i32)], i32); 15] = [
    // Below the first bench: a narrows near the sea, a hooked head.
    (&[(-8, 36), (10, 36), (26, 40)], 4),
    (&[(26, 40), (42, 41)], 2),
    (&[(42, 41), (56, 40), (70, 44), (82, 46)], 3),
    (&[(82, 46), (90, 45), (96, 40)], 2),
    // Below the second: rising inland, with a side arm into the second bench.
    (&[(-8, 106), (12, 103), (30, 98)], 4),
    (&[(30, 98), (50, 94), (66, 92), (80, 88), (90, 85)], 3),
    (&[(66, 92), (74, 81), (80, 75)], 2),
    // Below the third: the longest, falling inland, with a narrows and a forked head.
    (&[(-8, 140), (12, 142), (26, 146)], 4),
    (&[(26, 146), (40, 148)], 2),
    (&[(40, 148), (56, 150), (72, 156), (86, 157)], 3),
    (&[(86, 157), (93, 150), (96, 143)], 2),
    (&[(86, 157), (94, 162)], 2),
    // Below the fourth: winding up inland to a narrow head.
    (&[(-8, 216), (10, 213), (26, 207)], 4),
    (&[(26, 207), (42, 204), (58, 198), (72, 197), (84, 191)], 3),
    (&[(84, 191), (92, 185)], 2),
];

/// A short sound into the outer coast.
const SOUND: [(&[(i32, i32)], i32); 1] = [(&[(-8, 70), (8, 69), (18, 74)], 2)];

/// How far (x, y) lies from the water of the nearest fjord.
fn fjord_gap(p: &Painter, x: i32, y: i32) -> i32 {
    gap(p, x, y, &FJORDS).min(gap(p, x, y, &SOUND))
}

pub fn fjord_coast_biome(p: &Painter, x: i32, y: i32) -> Biome {
    // Green shores along the sea and down in the fjords; snow on the benches and the fjell.
    let coast = x.min(p.w - 1 - x);
    if coast < 32 + p.wob(x, y, 24, 4000, 8) || fjord_gap(p, x, y) < 10 + p.wob(x, y, 14, 4001, 5) {
        TEMPERATE
    } else {
        ARCTIC
    }
}

/// Fjord Coast, 256 x 256: a mountainous coast on the open sea (and its mirror across the
/// fjell). Long, narrow fjords branch deep inland between steep walls of snowy rock; grass,
/// farms and woods line the water, pines climb the slopes, and the broad benches between the
/// fjords are snowfields, one base to each. Fishing towns stand at the fjord heads; the roads go
/// round the heads to the high fjell between the coasts, where an icefield, tarns and crags lie
/// around the richest ore, and on each coast two fjords are bridged at narrows near the sea.
pub fn fjord_coast(p: &mut Painter) {
    // The open sea, in bays at the fjord mouths and round headlands between them; skerries
    // and wooded holms off the coast.
    p.paint(
        |p, x, y| {
            let headland = fjord_gap(p, x, y).clamp(0, 24) / 3;
            x.min(p.w - 1 - x) < 21 + p.wob(x, y, 18, 4002, 8) + p.wob(x, y, 6, 4003, 3) - headland
        },
        WATER,
    );
    for (x, y, r, s) in [
        (6, 22, 4, 4004),
        (4, 88, 5, 4006),
        (9, 126, 3, 4007),
        (5, 178, 4, 4008),
        (8, 238, 3, 4009),
    ] {
        p.blob(x, y, r, r / 2 + 1, s, |_, _, _| GRASS);
    }
    p.paint(
        |p, x, y| p.is_water(x, y) && x.min(p.w - 1 - x) > 1 && p.noise(x, y, 4, 4035) > 740,
        ROCK,
    );
    // The fjords, then the steep walls above a strip of shore, broken where side valleys
    // come down.
    p.paint(
        |p, x, y| fjord_gap(p, x, y) <= p.wob(x, y, 6, 4011, 1),
        WATER,
    );
    p.fill(
        |p, x, y| {
            let g = fjord_gap(p, x, y);
            let shore = ((p.noise(x, y, 8, 4012) - 420) / 120).clamp(0, 2) as i32;
            !p.is_water(x, y)
                && (shore < g && g <= shore + 3 + p.wob(x, y, 6, 4013, 1))
                && x.min(p.w - 1 - x) > 26
                && p.noise(x, y, 14, 4014) > 380
        },
        |p, x, y| {
            if p.noise(x, y, 4, 4036) > 640 {
                ROCK
            } else {
                SNOW_ROCK
            }
        },
    );
    // Crags on the benches and peaks on the fjell; an icefield and glaciers up there.
    p.paint(
        |p, x, y| {
            let crag = if x.min(p.w - 1 - x) > 100 {
                p.noise(x, y, 7, 4015) > 760
            } else {
                p.noise(x, y, 13, 4015) > 760
            };
            p.get(x, y) == SNOW && crag && !p.near_start(x, y, 22)
        },
        SNOW_ROCK,
    );
    for (x, y, r, s) in [(132, 38, 12, 4016), (114, 124, 6, 4017), (142, 98, 5, 4018)] {
        p.blob(x, y, r, 4, s, |p, x, y| {
            if [SNOW, SNOW_ROCK].contains(&p.get(x, y)) {
                ICE
            } else {
                p.get(x, y)
            }
        });
    }
    // Farms on the green shores; woods by the water, pines up the slopes, bare fjell above.
    farmland(p, 11, 4019, 680);
    p.fill(
        |p, x, y| {
            let tile = p.get(x, y);
            p.is_plain(tile)
                && !p.near_start(x, y, 14)
                && if tile == GRASS {
                    p.noise(x, y, 12, 4020) > 575
                } else {
                    fjord_gap(p, x, y) < 24 && p.noise(x, y, 10, 4021) > 640
                }
        },
        |p, x, y| p.biome(x, y).forest,
    );
    // The fishing towns at the fjord heads, and hamlets on the outer coast.
    p.city(
        |p, x, y| {
            [(101, 42), (96, 86), (97, 151), (97, 187)]
                .iter()
                .any(|&(cx, cy)| {
                    let (mx, my) = p.mirror_of(cx, cy);
                    let r = 6 + p.wob(x, y, 5, 4022, 1);
                    (x - cx).pow(2) + (y - cy).pow(2) <= r * r
                        || (x - mx).pow(2) + (y - my).pow(2) <= r * r
                })
        },
        101,
        42,
        3,
        4,
        4023,
    );
    for (x, y) in [
        (22, 26),
        (22, 58),
        (24, 112),
        (22, 168),
        (22, 226),
        (120, 72),
    ] {
        p.village(x, y, 3, 4024);
    }
    p.landmarks(22, 20, 4025, |p, x, _| x.min(p.w - 1 - x) > 20);
    // The road over the fjell, the roads along the benches round the fjord heads, and the two
    // bridges over the narrows.
    road(
        p,
        &[
            (118, 4),
            (122, 30),
            (116, 60),
            (124, 90),
            (120, 110),
            (127, 127),
        ],
    );
    road(p, &[(44, 19), (64, 16), (86, 18), (104, 22), (119, 14)]);
    road(p, &[(56, 69), (70, 64), (86, 62), (100, 64), (117, 62)]);
    road(
        p,
        &[(48, 121), (70, 122), (92, 124), (108, 120), (121, 111)],
    );
    road(
        p,
        &[(58, 175), (80, 176), (100, 172), (118, 176), (134, 176)],
    );
    road(
        p,
        &[(46, 229), (70, 232), (100, 226), (118, 214), (136, 208)],
    );
    road(p, &[(41, 21), (34, 30), (34, 52), (52, 68)]);
    road(p, &[(46, 122), (34, 134), (33, 160), (54, 175)]);
    // Ore: the richest on the fjell, more at the ends of the benches.
    for (x, y, r, s) in [
        (127, 127, 5, 4026),
        (120, 52, 4, 4027),
        (104, 118, 5, 4028),
        (112, 198, 4, 4029),
        (98, 70, 4, 4030),
        (94, 6, 4, 4031),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 4032);
    p.finish(1, 4033);
}

// ---------------------------------------------------------------------------------------------
// Tundra Steppe.

pub const TUNDRA_STEPPE: Layout = Layout {
    width: 256,
    height: 256,
    homes: &[(18, 16), (232, 20), (124, 14), (68, 88), (182, 92)],
};

pub fn tundra_steppe_biome(_: &Painter, _: i32, _: i32) -> Biome {
    ARCTIC
}

/// Doubled distance from (x, y) to the nearest of the tundra's three rivers: one meanders from
/// west to east through the middle, the other two (each the other's mirror) north and south.
fn tundra_river(p: &Painter, x: i32, y: i32) -> i32 {
    let (_, v) = p.centred(x, y);
    let main = 2 * p.meander(x, p.w, 44, 4100, 18) + 2 * p.meander(x, p.w, 11, 4101, 4);
    let side = 2 * p.meander(x, p.w, 36, 4102, 12) + 2 * p.meander(x, p.w, 9, 4103, 3);
    (v - main)
        .abs()
        .min((v - side - 156).abs())
        .min((v - side + 156).abs())
}

/// Tundra Steppe, 256 x 256: the endless flat tundra. A snow plain to the horizon, dotted with
/// countless frozen lakes drawn out in one direction and a few open thaw ponds; three frozen
/// rivers wind across it with thin taiga in their valleys, and long gravel eskers snake over the
/// plain. One dead-straight road runs to the mining town in the middle, with its open pit, spoil
/// heaps and tailings pond. Long sight lines: the lakes and eskers are the only cover.
pub fn tundra_steppe(p: &mut Painter) {
    let (cx, cy) = (p.w / 2, p.h / 2);
    // Frozen lakes, small and large, drawn out from north-west to south-east; a few thaw ponds.
    let lakes = (1, 1, 2, -2);
    p.paint(
        |p, x, y| {
            stretched(p, x, y, lakes, 12, 4104) > 700 || stretched(p, x, y, lakes, 26, 4105) > 735
        },
        ICE,
    );
    p.paint(
        |p, x, y| {
            p.get(x, y) == ICE
                && stretched(p, x, y, lakes, 12, 4104) > 760
                && p.noise(x, y, 18, 4106) > 620
        },
        WATER,
    );
    // The frozen rivers, with open water at the rapids.
    p.paint(|p, x, y| tundra_river(p, x, y) <= 3, ICE);
    p.paint(
        |p, x, y| tundra_river(p, x, y) <= 2 && p.noise(x, y, 8, 4107) > 760,
        WATER,
    );
    // Dry ground for every base.
    let homes: Vec<(i32, i32)> = p.starts.iter().copied().step_by(2).collect();
    for (k, &(sx, sy)) in homes.iter().enumerate() {
        p.blob(sx + 1, sy + 1, 14, 3, 4108 + k as u32, |_, _, _| SNOW);
    }
    // Eskers: long, low gravel ridges winding from south-west to north-east, with gaps.
    const HEADINGS: [(i32, i32); 5] = [(8, -1), (7, -4), (6, -6), (4, -7), (7, -2)];
    for (k, &(sx, sy)) in [
        (4, 92),
        (26, 206),
        (70, 252),
        (84, 150),
        (140, 236),
        (2, 160),
        (36, 54),
        (110, 196),
        (180, 250),
    ]
    .iter()
    .enumerate()
    {
        let (mut x, mut y, mut heading) = (sx, sy, k % HEADINGS.len());
        for step in 0..14 {
            let roll = hash(k as i32, step, 4113);
            heading = (heading + [0, 1, HEADINGS.len() - 1][(roll % 3) as usize]) % HEADINGS.len();
            let (dx, dy) = HEADINGS[heading];
            let next = (x + dx, y + dy);
            if !(roll >> 8).is_multiple_of(5) {
                p.stroke_with(&[(x, y), next], 1, |p, x, y| {
                    if p.get(x, y) == SNOW && !p.near_start(x, y, 16) {
                        SNOW_ROCK
                    } else {
                        p.get(x, y)
                    }
                });
            }
            (x, y) = next;
        }
    }
    // Thin taiga in the river valleys.
    p.paint(
        |p, x, y| {
            p.get(x, y) == SNOW
                && (6..40).contains(&tundra_river(p, x, y))
                && p.noise(x, y, 7, 4115) > 600
                && p.noise(x, y, 30, 4116) > 460
                && !p.near_start(x, y, 18)
        },
        SNOW_PINES,
    );
    // The mining town, its open pit, spoil heaps and tailings pond.
    p.city(
        |p, x, y| centre_d2(p, x, y) <= (16 + p.wob(x, y, 8, 4117, 3)).pow(2) && p.get(x, y) != ICE,
        cx,
        cy,
        6,
        5,
        4118,
    );
    p.stroke(&[(142, 106), (162, 114)], 8, DIRT);
    p.stroke(&[(164, 95), (172, 97), (178, 102)], 2, ROCK);
    p.stroke(&[(136, 96), (146, 92)], 1, ROCK);
    p.stroke(&[(166, 120), (170, 127)], 2, ROCK);
    p.blob(186, 116, 5, 3, 4123, |_, _, _| WATER);
    p.stroke(&[(141, 124), (148, 116)], 1, TRACK);
    // Herders' camps and a weather station.
    for (x, y) in [(44, 58), (198, 52), (100, 72), (160, 34), (20, 120)] {
        p.village(x, y, 3, 4124);
    }
    p.landmarks(24, 20, 4125, |p, x, y| centre_d2(p, x, y) > 30 * 30);
    // The road, dead straight from edge to edge through the town.
    road(p, &[(0, 102), (255, 153)]);
    // Ore: the pit, and fields out on the plain.
    for (x, y, r, s) in [
        (154, 111, 5, 4126),
        (128, 62, 5, 4127),
        (96, 40, 4, 4128),
        (160, 52, 4, 4129),
        (20, 72, 4, 4130),
        (234, 74, 4, 4131),
        (60, 124, 4, 4132),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 4133);
    p.finish(1, 4134);
}

// ---------------------------------------------------------------------------------------------
// Ice Shelf Rift.

pub const ICE_SHELF_RIFT: Layout = Layout {
    width: 240,
    height: 240,
    homes: &[(28, 34), (208, 36), (118, 30), (72, 76), (168, 60)],
};

/// Snow on the nunataks, with rock where other maps have trees: nothing grows here.
const POLAR: Biome = Biome {
    forest: SNOW_ROCK,
    ..ARCTIC
};

pub fn ice_shelf_rift_biome(_: &Painter, _: i32, _: i32) -> Biome {
    POLAR
}

/// The rifts in the shelf, each a line and its half-width (with its mirror): the long rift from
/// the western edge, wide in the middle and narrowing to its tip, and two cracks in from the ice
/// front.
const RIFTS: [(&[(i32, i32)], i32); 8] = [
    (&[(-4, 116), (10, 113), (22, 115), (36, 110), (50, 111)], 3),
    (
        &[
            (50, 111),
            (64, 106),
            (80, 105),
            (94, 100),
            (108, 102),
            (122, 99),
            (138, 100),
        ],
        4,
    ),
    (&[(138, 100), (150, 96), (162, 95), (174, 89)], 3),
    (&[(174, 89), (182, 85), (189, 78)], 1),
    (&[(78, 2), (81, 10), (79, 18), (84, 26)], 2),
    (&[(84, 26), (85, 32), (88, 38)], 1),
    (&[(162, 2), (159, 9), (161, 15), (157, 22)], 2),
    (&[(157, 22), (156, 30)], 1),
];

/// A nunatak around (x, y) and its mirror: snow on a dome of radius `r` and two lobes.
fn nunatak(p: &mut Painter, x: i32, y: i32, r: i32, seed: u32) {
    const DIRECTIONS: [(i32, i32); 8] = [
        (10, 0),
        (7, 7),
        (0, 10),
        (-7, 7),
        (-10, 0),
        (-7, -7),
        (0, -10),
        (7, -7),
    ];
    p.blob(x, y, r, r / 4 + 1, seed, |_, _, _| SNOW);
    for k in 0..2 {
        let (dx, dy) = DIRECTIONS[(hash(x, y, seed + k) % 8) as usize];
        let lobe = r * 2 / 3;
        let (lx, ly) = (x + dx * r / 14, y + dy * r / 14);
        p.blob(lx, ly, lobe, lobe / 2 + 1, seed + 10 + k, |_, _, _| SNOW);
    }
}

/// Ice Shelf Rift, 240 x 240: a floating ice shelf. A vast sheet of ice, which armies cross
/// anywhere and nobody builds on, is studded with nunataks: rocky islands of snow, ringed and
/// ridged with crags, where the bases, the ore and the research stations are. Two long rifts of
/// open water cut it from the edges, overlapping in the middle, with fields of crevasses about
/// them; they are crossed by four broad ice bridges and passable round their tips. The ice front
/// on the open sea is cracked and strewn with bergs.
pub fn ice_shelf_rift(p: &mut Painter) {
    let (cx, cy) = (p.w / 2, p.h / 2);
    p.paint(|_, _, _| true, ICE);
    // The open sea beyond the ice front, tabular bergs calved from it, and bergs aground.
    p.paint(
        |p, x, y| y.min(p.h - 1 - y) < 10 + p.wob(x, y, 30, 4200, 6) + p.wob(x, y, 8, 4201, 2),
        WATER,
    );
    p.paint(
        |p, x, y| p.is_water(x, y) && y.min(p.h - 1 - y) > 1 && p.noise(x, y, 6, 4202) > 720,
        ICEBERG,
    );
    p.paint(
        |p, x, y| {
            p.get(x, y) == ICE
                && y.min(p.h - 1 - y) < 18
                && p.noise(x, y, 4, 4203) > 780
                && !p.near_start(x, y, 24)
        },
        ICEBERG,
    );
    for (x, y, w, h) in [
        (30, 3, 7, 3),
        (102, 5, 4, 2),
        (182, 3, 6, 4),
        (224, 6, 3, 2),
    ] {
        p.paint(
            |p, cx, cy| {
                let (kx, ky) = canonical(p, cx, cy);
                p.is_water(cx, cy) && (x..x + w).contains(&kx) && (y..y + h).contains(&ky)
            },
            ICEBERG,
        );
    }
    // The nunataks: one under every base, the station's in the middle, ridges and lone peaks in
    // the open.
    let homes: Vec<(i32, i32)> = p.starts.iter().copied().step_by(2).collect();
    for (k, &(sx, sy)) in homes.iter().enumerate() {
        nunatak(p, sx + 1, sy + 1, 19, 4204 + k as u32);
    }
    nunatak(p, cx, cy, 8, 4210);
    nunatak(p, 214, 100, 8, 4211);
    nunatak(p, 22, 80, 7, 4212);
    p.stroke(&[(110, 64), (122, 68), (132, 74)], 6, SNOW);
    for (x, y, r, s) in [
        (60, 44, 2, 4227),
        (98, 58, 1, 4228),
        (146, 40, 2, 4229),
        (196, 70, 1, 4230),
        (232, 118, 2, 4231),
        (70, 132, 1, 4232),
    ] {
        p.blob(x, y, r, 1, s, |_, _, _| SNOW_ROCK);
    }
    // Bare rock and snowy crags round the rim of every nunatak, with gaps, and ridges of peaks
    // across them.
    let crag = |p: &Painter, x: i32, y: i32| {
        if p.noise(x, y, 4, 4238) > 570 {
            ROCK
        } else {
            SNOW_ROCK
        }
    };
    p.fill(
        |p, x, y| {
            p.get(x, y) == SNOW
                && (-2..=2).any(|d| {
                    [(x + d, y), (x, y + d)]
                        .iter()
                        .any(|&(a, b)| p.inside(a, b) && p.get(a, b) == ICE)
                })
                && p.noise(x, y, 5, 4214) > 420
        },
        crag,
    );
    p.fill(
        |p, x, y| {
            p.get(x, y) == SNOW
                && ((494..534).contains(&p.noise(x, y, 14, 4215)) || p.noise(x, y, 6, 4233) > 760)
                && !p.near_start(x, y, 10)
        },
        crag,
    );
    // The rifts, ragged at the edges and wider and narrower by turns, with floes adrift.
    p.paint(
        |p, x, y| {
            y.min(p.h - 1 - y) > 6
                && gap(p, x, y, &RIFTS) <= p.wob(x, y, 12, 4216, 3) + p.wob(x, y, 4, 4234, 1)
        },
        WATER,
    );
    p.paint(
        |p, x, y| p.is_water(x, y) && y.min(p.h - 1 - y) > 24 && p.noise(x, y, 3, 4235) > 790,
        ICEBERG,
    );
    // Crevasse fields beside the long rift: rows of open slits along it, broken every few cells.
    p.paint(
        |p, x, y| {
            let (kx, ky) = canonical(p, x, y);
            p.get(x, y) == ICE
                && (4..16).contains(&gap(p, x, y, &RIFTS[..4]))
                && p.noise(x, y, 14, 4236) > 560
                && (ky + p.wob(kx, ky, 10, 4237, 3)).rem_euclid(4) == 0
                && p.noise(x, y, 3, 4239) > 440
        },
        WATER,
    );
    // Research stations.
    p.city(|p, x, y| centre_d2(p, x, y) <= 5 * 5, cx, cy, 2, 4, 4217);
    for (x, y) in [(124, 70), (214, 98), (22, 80)] {
        p.village(x, y, 3, 4218);
    }
    p.landmarks(20, 18, 4219, |_, _, _| true);
    // The ice bridges over the long rifts.
    let hold = |p: &Painter, x: i32, y: i32| {
        if [WATER, ICEBERG].contains(&p.get(x, y)) {
            ICE
        } else {
            p.get(x, y)
        }
    };
    p.stroke_with(&[(40, 98), (40, 126)], 7, hold);
    p.stroke_with(&[(128, 86), (128, 114)], 7, hold);
    // Ore on the nunataks.
    for (x, y, r, s) in [
        (cx, cy, 4, 4220),
        (118, 68, 4, 4221),
        (214, 100, 5, 4222),
        (22, 80, 4, 4223),
    ] {
        p.ore(x, y, r, s);
    }
    // Gems in the ridge's field too: the fields on the small nunataks are mostly too small.
    p.gem_seam(118, 68);
    p.home_ore(4, 4224);
    p.finish(1, 4225);
}
