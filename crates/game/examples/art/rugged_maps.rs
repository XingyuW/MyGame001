//! Maps of broken ground, each shaped by one landform: `river_gorge` (a plateau split by a
//! gorge), `twin_volcanoes` (two volcanoes over a jungle valley), `crater_field` (a steppe pocked
//! by meteorite craters) and `highland_plateau` (tablelands stepping down by escarpments). The
//! landforms are painted by distance: from a gorge's course, a crater's centre, a step's edge.
//!
//! Seeds 6000 to 6999 are this file's.
use super::*;

// ---------------------------------------------------------------------------------------------
// Shared tools.

/// Squared distance from (x, y) to the segment from `a` to `b`, in cells.
fn segment_d2(x: i32, y: i32, (ax, ay): (i32, i32), (bx, by): (i32, i32)) -> i64 {
    let (dx, dy) = (i64::from(bx - ax), i64::from(by - ay));
    let (px, py) = (i64::from(x - ax), i64::from(y - ay));
    let len2 = dx * dx + dy * dy;
    let t = px * dx + py * dy;
    if len2 == 0 || t <= 0 {
        px * px + py * py
    } else if t >= len2 {
        let (qx, qy) = (i64::from(x - bx), i64::from(y - by));
        qx * qx + qy * qy
    } else {
        let cross = px * dy - py * dx;
        cross * cross / len2
    }
}

/// Distance in whole cells from every cell of the map to a path and to its point mirror, row
/// by row: the same for a cell and its mirror.
fn path_distances(p: &Painter, path: &[(i32, i32)]) -> Vec<i32> {
    let mirrored: Vec<(i32, i32)> = path.iter().map(|&(x, y)| p.mirror_of(x, y)).collect();
    (0..p.h)
        .flat_map(|y| (0..p.w).map(move |x| (x, y)))
        .map(|(x, y)| {
            let d2 = path
                .windows(2)
                .chain(mirrored.windows(2))
                .map(|s| segment_d2(x, y, s[0], s[1]))
                .min()
                .unwrap_or(i64::MAX);
            d2.isqrt() as i32
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// River Gorge.

pub const RIVER_GORGE: Layout = Layout {
    width: 256,
    height: 256,
    homes: &[(29, 29), (125, 23), (221, 29), (25, 113), (75, 85)],
};

pub fn river_gorge_biome(_: &Painter, _: i32, _: i32) -> Biome {
    TEMPERATE
}

/// The gorge's course from the western edge to the centre; the eastern half is its mirror.
/// The gorge's course from the western edge to the centre; the eastern half is its mirror.
const GORGE: [(i32, i32); 11] = [
    (-8, 172),
    (14, 166),
    (32, 172),
    (44, 186),
    (62, 194),
    (78, 186),
    (82, 166),
    (90, 152),
    (106, 150),
    (118, 140),
    (127, 128),
];

/// River Gorge, 256 x 256: a high limestone plateau split from edge to edge by a winding gorge
/// (the Verdon, say). The river runs on its floor between gravel bars, under walls of rock ten
/// cells thick; forest hangs on both rims, and the plateau above is farmland, woods, villages and
/// tarns. The gorge is crossed in five places: a high bridge at the centre with a small town at
/// each end, a high bridge on each side of it, and near each end a ford at the foot of switchback
/// roads cut down both walls.
pub fn river_gorge(p: &mut Painter) {
    let w = p.w;
    let gd = path_distances(p, &GORGE);
    let at = move |x: i32, y: i32| gd[(y * w + x) as usize];
    // The gorge: the river on its floor, gravel bars and scrub beside it, walls of rock.
    p.fill(
        |_, _, _| true,
        |p, x, y| {
            let d = at(x, y);
            if d <= 3 + p.wob(x, y, 9, 6002, 2) {
                WATER
            } else if d <= 8 + p.wob(x, y, 9, 6003, 2) {
                if p.noise(x, y, 4, 6004) > 600 {
                    DIRT
                } else {
                    SAND
                }
            } else if d <= 19 + p.wob(x, y, 24, 6005, 4) + p.wob(x, y, 5, 6006, 2) {
                ROCK
            } else {
                GRASS
            }
        },
    );
    // Ragged walls: buttresses and bays along both rims.
    for round in 0..2 {
        let ragged: Vec<(i32, i32, u32)> = (0..p.h)
            .flat_map(|y| (0..p.w).map(move |x| (x, y)))
            .filter_map(|(x, y)| {
                if p.get(x, y) != ROCK || p.noise(x, y, 3, 6007 + round) <= 620 {
                    return None;
                }
                [(1, 0), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .map(|&(dx, dy)| (x + dx, y + dy))
                    .find(|&(nx, ny)| p.inside(nx, ny) && [GRASS, SAND].contains(&p.get(nx, ny)))
                    .map(|(nx, ny)| (x, y, p.get(nx, ny)))
            })
            .collect();
        for (x, y, t) in ragged {
            p.set(x, y, t);
        }
    }
    // Forest along the rims, farmland and woods on the plateau, tarns.
    p.fill(
        |p, x, y| p.get(x, y) == GRASS && at(x, y) <= 32 && p.noise(x, y, 7, 6009) > 400,
        |_, _, _| FOREST,
    );
    farmland(p, 22, 6010, 600);
    p.forests(24, 6011, 640, |_, _| true);
    for (x, y, r, s) in [(114, 72, 4, 6012), (176, 70, 3, 6013), (42, 80, 3, 6014)] {
        p.blob(x, y, r, 2, s, |_, _, _| WATER);
    }
    // The bridge-head towns and the villages of the plateau.
    p.city(
        |p, x, y| {
            let (mx, my) = p.mirror_of(92, 112);
            let d = |cx: i32, cy: i32| (x - cx).pow(2) + (y - cy).pow(2) <= 9 * 9;
            (d(92, 112) || d(mx, my)) && p.get(x, y) != ROCK && !p.is_water(x, y)
        },
        92,
        112,
        3,
        5,
        6015,
    );
    for (x, y) in [(56, 58), (176, 44), (100, 36), (14, 136), (200, 104)] {
        p.village(x, y, 4, 6016);
    }
    p.landmarks(24, 16, 6017, |_, x, y| at(x, y) > 26);
    // The crossings, their approaches cleared of forest.
    let clear = |p: &Painter, x: i32, y: i32| match p.get(x, y) {
        FOREST | ORCHARD => GRASS,
        t => t,
    };
    p.stroke_with(&[(23, 118), (23, 142)], 10, clear);
    p.stroke_with(&[(63, 110), (63, 141)], 10, clear);
    p.stroke_with(&[(101, 80), (101, 102)], 10, clear);
    // A ford at the foot of switchback roads near each end of the gorge: a broad cut down each
    // wall and a gravel bar over the river.
    p.stroke_with(&[(23, 140), (23, 198)], 8, |p, x, y| match p.get(x, y) {
        WATER => SAND,
        ROCK | FOREST => DIRT,
        t => t,
    });
    p.stroke_with(
        &[
            (22, 136),
            (30, 144),
            (17, 150),
            (30, 158),
            (20, 168),
            (29, 180),
            (16, 188),
            (26, 198),
        ],
        1,
        |p, x, y| if p.is_water(x, y) { SAND } else { TRACK },
    );
    // High bridges: one at the centre and one on each side.
    let span = |p: &Painter, x: i32, y: i32| {
        if at(x, y) <= 25 {
            p.road_tile(x, y)
        } else {
            clear(p, x, y)
        }
    };
    p.stroke_with(&[(101, 101), (154, 154)], 6, span);
    p.stroke_with(&[(63, 141), (108, 174)], 5, span);
    // Roads over the plateau.
    for path in [
        &[
            (22, 30),
            (60, 60),
            (100, 52),
            (140, 58),
            (180, 50),
            (222, 30),
        ][..],
        &[(22, 32), (20, 70), (24, 110), (22, 136)][..],
        &[(60, 60), (68, 100), (63, 141)][..],
        &[(100, 52), (96, 80), (101, 101)][..],
        &[(108, 174), (116, 197)][..],
    ] {
        p.stroke_with(path, 1, |p, x, y| p.road_tile(x, y));
    }
    // Ore: on the fords' gravel, by the bridge heads, between the bases.
    for (x, y, r, s) in [
        (23, 168, 3, 6020),
        (82, 120, 5, 6021),
        (124, 88, 4, 6022),
        (150, 72, 4, 6023),
        (44, 112, 4, 6024),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 6040);
    p.finish(1, 6041);
}

// ---------------------------------------------------------------------------------------------
// Twin Volcanoes.

pub const TWIN_VOLCANOES: Layout = Layout {
    width: 256,
    height: 256,
    homes: &[(29, 29), (221, 33), (125, 21), (75, 71), (179, 75)],
};

/// The western volcano's crater; the eastern one is its mirror.
const CONE: (i32, i32) = (66, 128);

/// Distance in cells from the nearer of the two craters.
fn cone_d(p: &Painter, x: i32, y: i32) -> i32 {
    let (mx, my) = p.mirror_of(CONE.0, CONE.1);
    let west = (x - CONE.0).pow(2) + (y - CONE.1).pow(2);
    let east = (x - mx).pow(2) + (y - my).pow(2);
    west.min(east).isqrt()
}

/// Jungle down to beaches of black volcanic sand.
const JUNGLE_ISLE: Biome = Biome {
    shore: ASH,
    ..TROPIC
};

pub fn twin_volcanoes_biome(p: &Painter, x: i32, y: i32) -> Biome {
    if cone_d(p, x, y) <= 44 + p.wob(x, y, 14, 6100, 6) {
        VOLCANIC
    } else {
        JUNGLE_ISLE
    }
}

/// Twin Volcanoes, 256 x 256: two volcanoes rising from a jungle island between seas on its
/// western and eastern edges (Java, say). Each is a cone of ash ribbed with basalt around a crater
/// of lava; one flow runs down to the sea, another into the jungle, crossed only where they have
/// cooled under the roads. Between the cones lies a green valley with a town, lakes and terraced
/// fields climbing both flanks, and beaches of black sand line the coasts. The fight is in the
/// valley and round the mountains' shoulders.
pub fn twin_volcanoes(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    let (cx, cy) = (w / 2, h / 2);
    // How far up the mountain a cell lies: the distance from the crater, made uneven by spurs
    // and gullies.
    let slope = |p: &Painter, x: i32, y: i32| {
        cone_d(p, x, y) + p.wob(x, y, 11, 6117, 5) + p.wob(x, y, 4, 6118, 2)
    };
    // The sea along the western and eastern edges, and its beaches of black sand.
    p.paint(
        |p, x, y| x.min(p.w - 1 - x) < 16 + p.wob(x, y, 34, 6102, 9) + p.wob(x, y, 8, 6103, 2),
        WATER,
    );
    p.paint(
        |p, x, y| {
            let d = 2 + i32::from(p.noise(x, y, 6, 6119) > 560);
            !p.is_water(x, y) && (-d..=d).any(|dy| (-d..=d).any(|dx| p.is_water(x + dx, y + dy)))
        },
        ASH,
    );
    // Jungle on the lowlands, thin woods on the ash, palms behind the beaches.
    p.fill(
        |p, x, y| {
            let threshold = if p.biome(x, y).plain == ASH { 730 } else { 560 };
            p.get(x, y) == p.biome(x, y).plain && p.noise(x, y, 15, 6104) > threshold
        },
        |p, x, y| p.biome(x, y).forest,
    );
    p.paint(
        |p, x, y| {
            p.get(x, y) == ASH
                && p.biome(x, y).plain == GRASS
                && p.noise(x, y, 4, 6105) > 680
                && (-2..=2).all(|d| !p.is_water(x + d, y) && !p.is_water(x, y + d))
        },
        PALMS,
    );
    p.paint(
        |p, x, y| {
            p.get(x, y) == GRASS
                && p.noise(x, y, 5, 6120) > 560
                && (-6..=6).any(|d| p.is_water(x + d, y) || p.is_water(x, y + d))
        },
        PALMS,
    );
    // Terraced fields on the flanks that face the valley, where the jungle has been cleared.
    p.fill(
        |p, x, y| {
            let d = slope(p, x, y);
            let (u, _) = p.centred(x, y);
            // Towards the valley: east of the western crater, west of the eastern one.
            let reach = w - 1 - 2 * CONE.0;
            let inward = u.abs() < reach && reach - u.abs() > 2 * (cone_d(p, x, y) / 3);
            (26..=54).contains(&d)
                && inward
                && p.noise(x, y, 13, 6121) > 440
                && [GRASS, ASH, FOREST, ASH_WOODS].contains(&p.get(x, y))
        },
        |p, x, y| {
            if slope(p, x, y) / 3 % 2 == 0 {
                CROPS
            } else {
                DIRT
            }
        },
    );
    // The cones: ash slopes ribbed with basalt, the summit cone, the crater and its lava lake.
    p.paint(
        |p, x, y| {
            p.get(x, y) == ASH
                && (20..=40).contains(&slope(p, x, y))
                && (530..590).contains(&p.noise(x, y, 10, 6107))
                && p.noise(x, y, 24, 6108) > 400
        },
        BASALT,
    );
    p.paint(|p, x, y| slope(p, x, y) <= 20, BASALT);
    p.paint(
        |p, x, y| (11..=20).contains(&slope(p, x, y)) && p.noise(x, y, 3, 6109) > 690,
        ASH,
    );
    p.paint(
        |p, x, y| cone_d(p, x, y) <= 8 + p.wob(x, y, 6, 6110, 2),
        LAVA,
    );
    // Lava flows: one to the sea, one into the jungle (each with its mirror).
    let flow = |p: &Painter, x: i32, y: i32| {
        if p.is_water(x, y) { WATER } else { LAVA }
    };
    p.stroke_with(
        &[(60, 128), (48, 122), (36, 130), (22, 126), (-4, 131)],
        2,
        flow,
    );
    p.stroke_with(&[(62, 136), (54, 152), (54, 166), (46, 182)], 2, flow);
    // A crust of basalt along the flows.
    p.paint(
        |p, x, y| {
            p.is_plain(p.get(x, y))
                && p.noise(x, y, 3, 6111) > 540
                && [(1, 0), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .any(|&(dx, dy)| p.inside(x + dx, y + dy) && p.get(x + dx, y + dy) == LAVA)
        },
        BASALT,
    );
    // The valley town with a lake beside it, and the villages of the jungle and the coast.
    p.city(
        |p, x, y| centre_d2(p, x, y) <= (15 + p.wob(x, y, 8, 6112, 3)).pow(2),
        cx,
        cy,
        6,
        5,
        6113,
    );
    p.blob(140, 104, 5, 2, 6114, |_, _, _| WATER);
    for (x, y) in [
        (46, 56),
        (104, 40),
        (160, 44),
        (36, 84),
        (38, 174),
        (102, 96),
    ] {
        p.village(x, y, 4, 6115);
    }
    p.landmarks(22, 16, 6116, |p, x, y| cone_d(p, x, y) > 26);
    // Roads: down the valley, along the coast, across the north and over the mountain's
    // shoulder. Where a road meets a flow, the lava has cooled.
    let road = |p: &Painter, x: i32, y: i32| {
        if p.get(x, y) == LAVA {
            ASH
        } else {
            p.road_tile(x, y)
        }
    };
    for path in [
        &[(120, 6), (124, 40), (120, 76), (126, 104), (127, 128)][..],
        &[
            (30, 14),
            (26, 56),
            (30, 100),
            (26, 134),
            (32, 168),
            (26, 206),
            (32, 242),
        ][..],
        &[(27, 60), (52, 64), (84, 52), (122, 48)][..],
        &[(124, 44), (168, 58), (204, 50), (236, 58)][..],
        &[(118, 100), (92, 84), (64, 80), (40, 90), (29, 98)][..],
    ] {
        p.stroke_with(path, 1, road);
    }
    for (x, y) in [(27, 128), (54, 160)] {
        p.blob(x, y, 4, 1, 6122, |p, x, y| {
            if p.get(x, y) == LAVA {
                ASH
            } else {
                p.get(x, y)
            }
        });
    }
    // Ore: rich fields in the valley, ash fields on the mountains' shoulders, jungle fields.
    for (x, y, r, s) in [
        (108, 108, 5, 6123),
        (130, 82, 4, 6124),
        (64, 78, 4, 6125),
        (98, 156, 4, 6126),
        (38, 150, 4, 6127),
        (164, 100, 4, 6128),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 6140);
    p.finish(2, 6141);
}

// ---------------------------------------------------------------------------------------------
// Crater Field.

pub const CRATER_FIELD: Layout = Layout {
    width: 240,
    height: 240,
    homes: &[(29, 29), (205, 37), (117, 21), (63, 83), (165, 77)],
};

pub fn crater_field_biome(_: &Painter, _: i32, _: i32) -> Biome {
    TEMPERATE
}

/// What lies in a crater: fresh rubble ringed by ejecta, an old grassed bowl, or a lake.
#[derive(Clone, Copy, PartialEq)]
enum Crater {
    Young,
    Old,
    Lake,
}

/// A crater of radius `r` at (cx, cy) and its mirror: a floor, a raised rim of rock broken by
/// `gaps` breaches spread round it, and around a young one a blanket of ash thrown out by the
/// impact.
fn crater(p: &mut Painter, (cx, cy): (i32, i32), r: i32, kind: Crater, gaps: usize, seed: u32) {
    let (mx, my) = p.mirror_of(cx, cy);
    let first = hash(cx, cy, seed) as usize % 16;
    let half = 1 + r / 5;
    let thick = 1 + r / 10;
    let ejecta = if kind == Crater::Young {
        r * 3 / 4 + 3
    } else {
        0
    };
    let reach = r + thick + ejecta + 2;
    for (ox, oy, sign) in [(cx, cy, 1), (mx, my, -1)] {
        for y in oy - reach..=oy + reach {
            for x in ox - reach..=ox + reach {
                if !p.inside(x, y) {
                    continue;
                }
                // Offsets as seen from the crater's own side, so the mirror is painted alike.
                let (dx, dy) = (sign * (x - ox), sign * (y - oy));
                let d = (dx * dx + dy * dy).isqrt() + p.wob(x, y, 5, seed, 1);
                let breach = (0..gaps).any(|i| {
                    let (gx, gy) = RING[(first + i * 16 / gaps) % 16];
                    dx * gx + dy * gy > 0 && (dx * gy - dy * gx).abs() <= half * 9
                });
                let tile = p.get(x, y);
                let new = if d < r - thick {
                    match kind {
                        Crater::Young => DIRT,
                        Crater::Old if p.noise(x, y, 5, seed + 1) > 640 => FOREST,
                        Crater::Old => GRASS,
                        Crater::Lake if d < r - thick - 1 => WATER,
                        Crater::Lake => SAND,
                    }
                } else if d <= r + thick {
                    if breach { DIRT } else { ROCK }
                } else if d <= r + thick + ejecta
                    && p.is_plain(tile)
                    && p.noise(x, y, 3, seed + 2)
                        > i64::from(330 + 400 * (d - r - thick) / ejecta.max(1))
                {
                    ASH
                } else {
                    tile
                };
                p.set(x, y, new);
            }
        }
    }
}

/// Crater Field, 240 x 240: a steppe pocked by meteorite craters. Craters of every size lie over
/// the grass and bare earth, each a bowl in a raised rim of rock broken by gaps: young ones with
/// floors of rubble and blankets of ash thrown out around them, old ones grassed over, some
/// flooded into lakes. A great crater fills the middle, its rim breached in six places and its
/// floor rich in ore around a central peak; villages and dirt roads lie between.
pub fn crater_field(p: &mut Painter) {
    // The steppe: grass, bare earth, a few birch copses.
    p.paint(|p, x, y| p.noise(x, y, 18, 6200) > 590, DIRT);
    p.paint(
        |p, x, y| p.get(x, y) == DIRT && p.noise(x, y, 5, 6201) > 640,
        GRASS,
    );
    p.forests(9, 6202, 740, |_, _| true);
    // The craters, the old ones first: younger impacts fall across them.
    for (x, y, r, kind, gaps, s) in [
        (24, 92, 12, Crater::Old, 4, 6203),
        (199, 115, 8, Crater::Old, 4, 6204),
        (160, 30, 9, Crater::Lake, 2, 6205),
        (206, 92, 11, Crater::Lake, 3, 6206),
        (90, 30, 4, Crater::Old, 2, 6207),
        (72, 44, 10, Crater::Young, 3, 6208),
        (110, 68, 8, Crater::Young, 3, 6209),
        (140, 50, 5, Crater::Young, 2, 6210),
        (186, 62, 4, Crater::Young, 2, 6211),
        (12, 170, 5, Crater::Young, 2, 6226),
        (150, 160, 4, Crater::Old, 3, 6227),
    ] {
        crater(p, (x, y), r, kind, gaps, s);
    }
    // The great crater in the middle: a ring of hummocky ground around its rim, the rim with
    // six breaches, a floor of rubble and a central peak.
    let ring = |p: &Painter, x: i32, y: i32| {
        let (u, v) = p.centred(x, y);
        ((u * u + v * v).isqrt() / 2 + p.wob(x, y, 9, 6212, 2), u, v)
    };
    p.fill(
        |p, x, y| {
            let (d, _, _) = ring(p, x, y);
            (40..=50).contains(&d) && p.is_plain(p.get(x, y)) && p.noise(x, y, 4, 6213) > 520
        },
        |_, _, _| DIRT,
    );
    p.fill(
        |p, x, y| ring(p, x, y).0 <= 39,
        |p, x, y| {
            let (d, u, v) = ring(p, x, y);
            let breach = [1, 4, 7, 9, 12, 15].iter().any(|&k| {
                let (gx, gy) = RING[k];
                u * gx + v * gy > 0 && (u * gy - v * gx).abs() <= 2 * 7 * 9
            });
            if d >= 34 {
                if breach { DIRT } else { ROCK }
            } else if d <= 5 + p.wob(x, y, 4, 6214, 2) {
                ROCK
            } else if p.noise(x, y, 5, 6215) > 600 {
                GRASS
            } else {
                DIRT
            }
        },
    );
    // Villages on the steppe.
    for (x, y) in [(46, 66), (150, 98), (100, 44), (190, 18), (20, 130)] {
        p.village(x, y, 4, 6216);
    }
    p.landmarks(22, 16, 6217, |p, x, y| centre_d2(p, x, y) > 42 * 42);
    // Roads between the craters, and in through the breaches.
    let road = |p: &Painter, x: i32, y: i32| p.road_tile(x, y);
    for path in [
        &[(30, 50), (46, 66), (64, 76), (96, 92)][..],
        &[(118, 34), (126, 56), (128, 82)][..],
        &[(166, 70), (150, 98), (140, 100)][..],
        &[(206, 50), (226, 80), (226, 160), (210, 200)][..],
        &[(40, 20), (90, 16), (190, 18), (206, 30)][..],
    ] {
        p.stroke_with(path, 1, road);
    }
    // Ore: the rich floor of the great crater, fields in the old craters and on the steppe.
    for (x, y, r, s) in [
        (100, 110, 5, 6220),
        (124, 98, 5, 6221),
        (24, 92, 4, 6222),
        (199, 115, 3, 6223),
        (150, 74, 4, 6224),
        (40, 128, 4, 6225),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 6240);
    p.finish(1, 6241);
}

// ---------------------------------------------------------------------------------------------
// Highland Plateau.

pub const HIGHLAND_PLATEAU: Layout = Layout {
    width: 256,
    height: 256,
    homes: &[(29, 29), (104, 20), (20, 104), (90, 90), (210, 24)],
};

/// Which step of the highland a cell stands on: 3 on the high spine that runs from the
/// north-east corner to the south-west one, down to 0 on the desert plains in the other two
/// corners.
fn highland_level(p: &Painter, x: i32, y: i32) -> i32 {
    let a = (x + y - (p.w - 1)).abs() + p.wob(x, y, 40, 6300, 18) + p.wob(x, y, 11, 6301, 5);
    match a {
        ..46 => 3,
        46..102 => 2,
        102..160 => 1,
        _ => 0,
    }
}

/// The dry middle step: bare red earth and scrub under sandstone cliffs.
const DRY_STEP: Biome = Biome {
    plain: DIRT,
    cliff: MESA,
    ..TEMPERATE
};

pub fn highland_plateau_biome(p: &Painter, x: i32, y: i32) -> Biome {
    match highland_level(p, x, y) {
        0 => DESERT_BIOME,
        1 => DRY_STEP,
        _ => TEMPERATE,
    }
}

/// Highland Plateau, 256 x 256: tablelands stepping down by escarpments (the Ethiopian
/// highlands, say). A summit plateau of moorland, woods and lakes runs from the north-east corner
/// to the south-west one inside a rim of dark basalt; below it lies a step of grain fields, then a
/// dry step of red earth and thorn scrub under rock walls, then desert plains with mesas and buttes
/// in the other two corners. Every escarpment is a long cliff, climbed only where a road ramps up
/// it; bases stand on every step.
pub fn highland_plateau(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    let levels: Vec<i32> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| highland_level(p, x, y))
        .collect();
    let level = move |x: i32, y: i32| levels[(y * w + x) as usize];
    // The escarpments: the rim of every step, three to five cells of cliff above the step below.
    p.fill(
        |p, x, y| {
            let here = level(x, y);
            let r = 3
                + i32::from(p.noise(x, y, 6, 6302) > 520)
                + i32::from(p.noise(x, y, 3, 6303) > 650);
            (-r..=r).any(|dy| {
                (-r..=r).any(|dx| {
                    dx * dx + dy * dy <= r * r
                        && p.inside(x + dx, y + dy)
                        && level(x + dx, y + dy) < here
                })
            })
        },
        // The summit's rim is the dark basalt of the old lava sheets the highland is made of.
        |p, x, y| {
            if level(x, y) == 3 {
                BASALT
            } else {
                p.biome(x, y).cliff
            }
        },
    );
    // Mesas and buttes standing on the plains below.
    for (x, y, r, s) in [
        (14, 14, 5, 6304),
        (60, 10, 3, 6305),
        (8, 64, 4, 6306),
        (54, 50, 2, 6307),
        (100, 46, 4, 6308),
        (40, 92, 3, 6309),
        (128, 8, 3, 6310),
    ] {
        p.blob(x, y, r, 2, s, |p, x, y| {
            if level(x, y) <= 1 { MESA } else { p.get(x, y) }
        });
    }
    p.paint(
        |p, x, y| level(x, y) <= 1 && p.is_plain(p.get(x, y)) && p.noise(x, y, 6, 6311) > 790,
        MESA,
    );
    // Highland lakes on the spine.
    for (x, y, r, s) in [(146, 108, 8, 6312), (196, 58, 5, 6313), (176, 92, 4, 6314)] {
        p.blob(x, y, r, 3, s, |_, _, _| WATER);
    }
    // Grain fields on the second step and in the hollows of the summit plateau, woods on the
    // high ground, thorn scrub on the dry step.
    p.paint(
        |p, x, y| {
            let n = p.noise(x, y, 15, 6315);
            p.get(x, y) == GRASS && ((level(x, y) == 2 && n > 520) || (level(x, y) == 3 && n > 690))
        },
        DIRT,
    );
    p.paint(
        |p, x, y| level(x, y) >= 2 && p.get(x, y) == DIRT && p.noise(x, y, 6, 6316) > 470,
        CROPS,
    );
    p.fill(
        |p, x, y| {
            let threshold = match level(x, y) {
                3 => 590,
                2 => 680,
                1 => 750,
                _ => 2000,
            };
            p.is_plain(p.get(x, y)) && p.noise(x, y, 13, 6317) > threshold
        },
        |_, _, _| FOREST,
    );
    for (x, y) in [
        (58, 20),
        (20, 58),
        (60, 134),
        (134, 60),
        (150, 34),
        (36, 150),
    ] {
        p.village(x, y, 4, 6318);
    }
    p.landmarks(22, 16, 6319, |_, _, _| true);
    // Roads, each climbing every escarpment it meets by a broad ramp.
    let ramp = |p: &Painter, x: i32, y: i32| {
        if [ROCK, MESA, BASALT].contains(&p.get(x, y)) {
            p.biome(x, y).plain
        } else {
            p.get(x, y)
        }
    };
    let road = |p: &Painter, x: i32, y: i32| p.road_tile(x, y);
    for path in [
        // Up the middle to the summit plateau.
        &[(30, 34), (60, 66), (96, 98), (127, 128)][..],
        // Along the western and northern edges, up to the spine in the corners.
        &[(30, 32), (22, 70), (22, 106), (30, 160), (46, 230)][..],
        &[(32, 30), (70, 22), (106, 22), (160, 30), (210, 26)][..],
        // From the bases on the dry step up to the spine.
        &[(22, 106), (60, 134), (96, 150), (112, 164)][..],
        &[(106, 22), (134, 60), (150, 96), (164, 112)][..],
    ] {
        p.stroke_with(path, 6, ramp);
        p.stroke_with(path, 1, road);
    }
    // Ore: on the summit plateau, the second step, the plains below.
    for (x, y, r, s) in [
        (116, 110, 5, 6320),
        (70, 110, 4, 6321),
        (110, 70, 4, 6322),
        (10, 40, 4, 6323),
        (200, 100, 4, 6324),
        (84, 160, 4, 6325),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 6340);
    p.finish(1, 6341);
}
