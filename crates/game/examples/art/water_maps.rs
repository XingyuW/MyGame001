//! Five maps shaped by water: a lagoon city of canals and bridges, polders won from the sea
//! behind straight dikes, a great inland sea ringed by varied coasts, a tropical coast of
//! mangroves and tidal creeks, and a high coast of granite cliffs above the sea. Each puts its
//! water where it changes how armies move: causeways and bridges, dikes and ditches with dams,
//! a ring of land with bridges across, mud belts on the flanks, cliffs with a few coves.
//!
//! Seeds 3000 to 3999 are this file's.
use super::*;

pub const CANAL_CITY: Layout = Layout {
    width: 256,
    height: 256,
    homes: &[(26, 26), (226, 28), (126, 20), (76, 40), (176, 40)],
};

pub fn canal_city_biome(_: &Painter, _: i32, _: i32) -> Biome {
    TEMPERATE
}

/// Canal City, 256 x 256: a city of canals on its islands in a lagoon. The lagoon runs across
/// the middle of the map between two shores of farmland, villages and rivers, where the bases
/// lie; long barrier islands with beaches and pinewoods close it off from the open sea on the
/// west and east, each cut by an inlet. The city is crossed by a winding grand canal and a web
/// of narrow canals that its streets bridge; causeways join it to both shores and the lidos.
pub fn canal_city(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    let (cx, cy) = (w / 2, h / 2);
    let edge = |p: &Painter, x: i32| x.min(p.w - 1 - x);
    // The lagoon across the middle, broadest around the city, and the open sea along the west
    // and east coasts.
    let lagoon = |p: &Painter, x: i32, y: i32| {
        let (u, v) = p.centred(x, y);
        let half = 54 - u * u / 2600 + p.wob(x, y, 40, 3002, 10) + p.wob(x, y, 9, 3001, 2);
        v.abs() < 2 * half
    };
    p.paint(
        |p, x, y| lagoon(p, x, y) || edge(p, x) < 8 + p.wob(x, y, 24, 3003, 3),
        WATER,
    );
    // The lidos: barrier islands between lagoon and sea, each cut by an inlet.
    let lido = |p: &Painter, x: i32, y: i32| edge(p, x) + p.wob(x, y, 16, 3004, 2);
    p.fill(
        |p, x, y| lagoon(p, x, y) && (10..=26).contains(&lido(p, x, y)),
        |p, x, y| if lido(p, x, y) < 14 { SAND } else { GRASS },
    );
    p.stroke(&[(0, 110), (36, 110)], 3, WATER);
    // The city's islands: an oval in the middle of the lagoon.
    let island = |p: &Painter, x: i32, y: i32| {
        let (u, v) = p.centred(x, y);
        let r = 152 + 2 * p.wob(x, y, 20, 3005, 6);
        u * u + (v * 76 / 33).pow(2) <= r * r
    };
    p.paint(island, GRASS);
    // The grand canal: a reversed S from tip to tip of the islands (doubled rows).
    let canal = |p: &Painter, x: i32, y: i32| {
        let (u, v) = p.centred(x, y);
        let swing = u * (152 * 152 - u * u) / 37_557;
        (v + swing + 2 * p.wob(x, y, 14, 3007, 2)).abs()
    };
    p.paint(|p, x, y| island(p, x, y) && canal(p, x, y) <= 7, WATER);
    // The city: narrow streets and blocks, a few campaniles at its heart.
    p.city(island, cx, cy, 14, 6, 3008);
    // The rii: canals two cells wide down the middle of the blocks, in runs of three blocks,
    // lined by houses on both banks; the streets run over them on low culverts (so that the
    // city is not a thicket of bridges, each with its repair huts).
    let rio = |p: &Painter, x: i32, y: i32| {
        let (u, v) = p.centred(x, y);
        let (bu, bv) = ((u + 6).div_euclid(12), (v + 6).div_euclid(12));
        let run = |b: i32| (b + 1).div_euclid(3);
        let active = |a: i32, b: i32, seed: u32| {
            let key = if (a, b) < (-a, -b) { (a, b) } else { (-a, -b) };
            hash(key.0, key.1, seed) % 5 < 2
        };
        let middle = |c: i32| [5, 7].contains(&c.rem_euclid(12));
        (middle(u) && active(bu, run(bv), 3009)) || (middle(v) && active(run(bu), bv, 3010))
    };
    p.fill(
        |p, x, y| island(p, x, y) && rio(p, x, y),
        |p, x, y| match p.get(x, y) {
            CITY | TOWER | PLAZA => WATER,
            tile => tile,
        },
    );
    // The great square on the grand canal, the arsenal's basin, and the public gardens.
    p.blob(118, 106, 4, 1, 3011, |p, x, y| {
        if p.is_water(x, y) { WATER } else { PLAZA }
    });
    p.fill(
        |_, x, y| (174..=188).contains(&x) && (110..=118).contains(&y),
        |_, x, y| {
            if (177..=185).contains(&x) && (112..=116).contains(&y) {
                WATER
            } else {
                PLAZA
            }
        },
    );
    for (x, y, r, s) in [(98, 112, 8, 3012), (60, 126, 4, 3013)] {
        p.blob(
            x,
            y,
            r,
            2,
            s,
            |p, x, y| {
                if p.is_water(x, y) { WATER } else { GRASS }
            },
        );
    }
    // A fishing island and the walled cemetery island out in the lagoon.
    p.blob(60, 98, 8, 3, 3027, |_, _, _| GRASS);
    p.village(60, 98, 6, 3028);
    p.fill(
        |_, x, y| (101..=108).contains(&x) && (89..=94).contains(&y),
        |_, x, y| {
            if (102..=107).contains(&x) && (90..=93).contains(&y) {
                FOREST
            } else {
                PLAZA
            }
        },
    );
    // Salt marshes in the open lagoon.
    p.paint(
        |p, x, y| {
            p.is_water(x, y)
                && lagoon(p, x, y)
                && !island(p, x, y)
                && edge(p, x) > 30
                && p.noise(x, y, 9, 3013) > 720
        },
        MARSH,
    );
    // The mainland: a patchwork of fields, meadows and vineyards, woods, two rivers running
    // into the lagoon.
    let mainland = move |y: i32| (y - cy).abs() > 46;
    p.fill(
        |p, x, y| mainland(y) && p.get(x, y) == GRASS,
        |p, x, y| {
            let (u, v) = p.centred(x, y);
            let (bu, bv) = ((u + 14).div_euclid(28), (v + 10).div_euclid(20));
            let key = if (bv, bu) < (-bv, -bu) {
                (bu, bv)
            } else {
                (-bu, -bv)
            };
            match hash(key.0, key.1, 3014) % 12 {
                0..=3 => CROPS,
                4 | 5 => DIRT,
                6 if !p.near_start(x, y, 24) => ORCHARD,
                _ => GRASS,
            }
        },
    );
    p.forests(18, 3016, 670, move |_, y| mainland(y));
    p.stroke(&[(48, 0), (52, 18), (46, 40), (52, 60), (48, 86)], 1, WATER);
    p.stroke(
        &[(152, 0), (148, 22), (154, 46), (150, 70), (154, 86)],
        1,
        WATER,
    );
    // Pinewoods along the lidos.
    p.paint(
        |p, x, y| {
            lagoon(p, x, y)
                && (14..=22).contains(&edge(p, x))
                && p.get(x, y) == GRASS
                && p.noise(x, y, 6, 3017) > 600
        },
        FOREST,
    );
    for (x, y) in [(100, 50), (204, 62), (60, 70), (16, 60)] {
        p.village(x, y, 4, 3018);
    }
    p.landmarks(22, 18, 3019, move |_, _, y| mainland(y));
    // Roads: the coast road on each shore, the roads to the bases, the lido roads.
    p.road_x(68, 10, w - 11, 2);
    p.road_y(27, 32, 68, 2);
    p.road_y(77, 46, 68, 2);
    p.road_y(177, 46, 68, 2);
    p.road_y(227, 34, 68, 2);
    p.road_y(16, 60, h - 61, 3);
    p.road_y(12, 104, 116, 9);
    // Causeways over the lagoon to the city, and the bridges over the grand canal.
    p.road_y(cx - 2, 26, cy, 4);
    p.road_y(86, 68, 140, 4);
    p.road_y(186, 68, 112, 4);
    p.road_x(cy - 2, 16, 64, 4);
    // Ore: in the fields between the bases, on the lidos and in the city's park.
    for (x, y, r, s) in [
        (100, 110, 5, 3020),
        (104, 58, 4, 3021),
        (150, 60, 4, 3022),
        (20, 120, 4, 3023),
        (238, 70, 4, 3024),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 3025);
    p.finish(2, 3026);
}

pub const POLDER_LOWLANDS: Layout = Layout {
    width: 256,
    height: 224,
    homes: &[(44, 24), (210, 28), (98, 34), (156, 30)],
};

pub fn polder_lowlands_biome(_: &Painter, _: i32, _: i32) -> Biome {
    TEMPERATE
}

/// Polder Lowlands, 256 x 224: land won from the sea, flat as a table and ruled with a straight
/// edge. Dunes and sea dikes hold back the sea on the west and east; behind them dikes enclose
/// polders of long strip fields, pasture and bulbs drained by ditches, with ribbon villages
/// along the dike roads. At the centre lies a reclaimed lake: a polder of big square fields and
/// a village, ringed by its canal with rows of windmills on the bank and entered by bridges. A
/// harbour town stands where the dikes meet each coast.
pub fn polder_lowlands(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    let (cx, cy) = (w / 2, h / 2);
    let edge = |p: &Painter, x: i32| x.min(p.w - 1 - x);
    // How far (doubled cells, Chebyshev) a cell lies outside the reclaimed lake's ring canal.
    let ring = |p: &Painter, x: i32, y: i32| {
        let (u, v) = p.centred(x, y);
        (u.abs() - 104).max(v.abs() - 80)
    };
    // The strip fields: crops, bulbs, plough and pasture, in parcels of 12 x 24 cells (in the
    // old polders) and 24 x 24 (on the reclaimed lake), and here and there a plantation wood.
    p.fill(
        |_, _, _| true,
        |p, x, y| {
            let (u, v) = p.centred(x, y);
            let across = if ring(p, x, y) < 0 { 48 } else { 24 };
            let (bu, bv) = ((u + across / 2).div_euclid(across), (v + 24).div_euclid(48));
            let key = if (bv, bu) < (-bv, -bu) {
                (bu, bv)
            } else {
                (-bu, -bv)
            };
            match hash(key.0, key.1, 3100) % 14 {
                0..=4 => CROPS,
                5 | 6 => DIRT,
                7 if !p.near_start(x, y, 24) => FOREST,
                _ => GRASS,
            }
        },
    );
    // The sea, the beach and the dunes along the west and east coasts.
    p.paint(
        |p, x, y| edge(p, x) < 14 + p.wob(x, y, 40, 3101, 4) + p.wob(x, y, 8, 3102, 1),
        WATER,
    );
    p.paint(
        |p, x, y| !p.is_water(x, y) && edge(p, x) < 24 + p.wob(x, y, 20, 3103, 2),
        SAND,
    );
    p.paint(
        |p, x, y| p.get(x, y) == SAND && edge(p, x) > 18 && p.noise(x, y, 6, 3104) > 590,
        FOREST,
    );
    // The ring canal around the reclaimed lake.
    p.paint(|p, x, y| (0..6).contains(&ring(p, x, y)), WATER);
    // Meres left unreclaimed, reeds round them.
    for (x, y, r, s) in [(128, 10, 7, 3105), (46, 86, 4, 3106)] {
        p.blob(x, y, r + 3, 2, s, |_, _, _| MARSH);
        p.blob(x, y, r, 2, s + 1, |_, _, _| WATER);
    }
    // Ditches between the strips, each broken by earth dams; across the fields on the lake.
    p.paint(
        |p, x, y| {
            let (u, v) = p.centred(x, y);
            let open = [CROPS, DIRT, GRASS].contains(&p.get(x, y))
                && edge(p, x) > 28
                && !p.near_start(x, y, 18);
            let old =
                ring(p, x, y) > 12 && (u + 12).rem_euclid(24) == 1 && (v + 18).rem_euclid(36) >= 8;
            let new =
                ring(p, x, y) < -10 && (v + 24).rem_euclid(24) == 1 && (u + 18).rem_euclid(40) >= 8;
            open && (old || new)
        },
        WATER,
    );
    // The dikes: sea dikes, and the dike roads between the polders and round the lake.
    let dike = |p: &Painter, x: i32, y: i32| {
        if p.is_water(x, y) { BRIDGE } else { TRACK }
    };
    p.stroke_with(&[(26, 0), (26, h - 1)], 1, dike);
    p.stroke_with(&[(26, 56), (w - 27, 56)], 1, dike);
    p.stroke_with(&[(64, 0), (64, h - 1)], 1, dike);
    p.stroke_with(&[(26, cy), (64, cy)], 1, dike);
    p.fill(|p, x, y| (-4..0).contains(&ring(p, x, y)), |_, _, _| TRACK);
    // Windmills in rows on the outer bank of the ring canal.
    p.paint(
        |p, x, y| {
            let (u, v) = p.centred(x, y);
            ring(p, x, y) == 9 && (u + v).rem_euclid(16) == 0 && !p.is_water(x, y)
        },
        COTTAGE,
    );
    // Farmsteads along the dike roads.
    p.paint(
        |p, x, y| {
            let (kx, ky) = canonical(p, x, y);
            [CROPS, DIRT, GRASS].contains(&p.get(x, y))
                && !p.near_start(x, y, 16)
                && [(1, 0), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .any(|&(dx, dy)| p.inside(x + dx, y + dy) && p.get(x + dx, y + dy) == TRACK)
                && hash(kx, ky, 3118).is_multiple_of(13)
        },
        FARMHOUSE,
    );
    // The harbour town on each coast, with its basin.
    p.city(
        |p, x, y| {
            let (mx, my) = p.mirror_of(30, 80);
            [(30, 80), (mx, my)]
                .iter()
                .any(|&(tx, ty)| (x - tx).pow(2) * 2 + (y - ty).pow(2) <= 13 * 13)
                && !p.is_water(x, y)
        },
        30,
        80,
        6,
        5,
        3107,
    );
    p.fill(
        |_, x, y| (10..=30).contains(&x) && (78..=82).contains(&y),
        |_, _, _| WATER,
    );
    // The village on the lake, and ribbon villages along the dikes.
    p.city(
        |p, x, y| centre_d2(p, x, y) <= (12 + p.wob(x, y, 6, 3108, 2)).pow(2),
        cx,
        cy,
        4,
        5,
        3109,
    );
    for (x, y) in [(40, 58), (120, 54), (200, 58), (66, 96), (160, 90)] {
        p.village(x, y, 3, 3110);
    }
    // Roads: the highways through the lake polder and its bridges, roads to the bases.
    p.road_x(cy - 2, 26, w - 27, 4);
    p.road_y(cx - 2, 20, cy, 4);
    p.road_y(96, 56, 100, 3);
    p.road_y(158, 56, 100, 3);
    p.road_x(88, 76, 180, 2);
    p.road_y(45, 30, 56, 2);
    p.road_y(99, 40, 56, 2);
    p.road_y(157, 36, 56, 2);
    p.road_y(211, 34, 56, 2);
    // Ore: on the reclaimed lake, in the old polders on the flanks and between the bases.
    for (x, y, r, s) in [
        (104, 98, 5, 3111),
        (150, 74, 4, 3112),
        (44, 98, 4, 3113),
        (70, 30, 4, 3114),
        (128, 40, 4, 3115),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 3116);
    p.finish(1, 3117);
}

pub const INLAND_SEA: Layout = Layout {
    width: 256,
    height: 256,
    homes: &[(24, 24), (226, 30), (96, 20), (160, 22), (20, 120)],
};

pub fn inland_sea_biome(p: &Painter, x: i32, y: i32) -> Biome {
    // Snow on the high peaks of the mountains in the north-east and the south-west.
    let (mx, my) = p.mirror_of(x, y);
    let r = 14 + p.wob(x, y, 10, 3200, 4);
    let peak = |x: i32, y: i32| (x - 244).pow(2) + (y - 80).pow(2) <= r * r;
    if peak(x, y) || peak(mx, my) {
        ARCTIC
    } else {
        TEMPERATE
    }
}

/// How far (doubled cells, the north-south axis stretched) a cell lies from the middle of the
/// inland sea, whose basin is broader east to west.
fn basin(p: &Painter, x: i32, y: i32) -> i32 {
    let (u, v) = p.centred(x, y);
    let v = v * 84 / 70;
    (u * u + v * v).isqrt()
}

/// Inland Sea, 256 x 256: a great landlocked sea and the ring of country around it. A peninsula
/// with a mountain spine and cliffs on its south coast reaches in from the north, and from the
/// south its twin; long bridges join both to the island fortress in the middle. Round the shore
/// lie steppe and farmland, port towns on sandy bays, a river delta of reeds and channels, and
/// snow-capped mountains that fall to the sea in cliffs. The bases stand on the ring, with a way
/// round the sea either side and the bridges across.
pub fn inland_sea(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    let (cx, cy) = (w / 2, h / 2);
    // The sea with its gulfs, the peninsulas and the island. `offshore` is how far (doubled
    // cells) a cell lies out from the shore.
    let offshore = |p: &Painter, x: i32, y: i32| {
        166 + 2 * (p.wob(x, y, 56, 3201, 12) + p.wob(x, y, 16, 3202, 3)) - basin(p, x, y)
    };
    p.paint(
        |p, x, y| offshore(p, x, y) + 2 * p.wob(x, y, 6, 3231, 1) > 0,
        WATER,
    );
    for (x, y, r, s) in [(186, 58, 11, 3232), (180, 68, 9, 3234), (84, 62, 9, 3233)] {
        p.blob(x, y, r, 3, s, |_, _, _| WATER);
    }
    p.stroke(&[(124, 40), (125, 68)], 3, GRASS);
    p.fill(
        |p, x, y| {
            let (kx, ky) = canonical(p, x, y);
            let (dx, dy) = (kx - 127 + (ky - 80) / 3, ky - 80);
            dx * dx / 2 + dy * dy <= (16 + p.wob(x, y, 10, 3203, 5)).pow(2)
        },
        |_, _, _| GRASS,
    );
    p.blob(cx, cy, 13, 3, 3204, |_, _, _| GRASS);
    // Steppe and farmland round the shore, woods.
    p.fill(
        |p, x, y| p.get(x, y) == GRASS && p.noise(x, y, 18, 3205) > 560,
        |p, x, y| {
            if p.noise(x, y, 6, 3206) > 520 {
                CROPS
            } else {
                DIRT
            }
        },
    );
    p.forests(16, 3207, 650, |_, _| true);
    p.stroke_with(&[(124, 40), (125, 70)], 4, |p, x, y| {
        if p.is_water(x, y) { WATER } else { GRASS }
    });
    // The mountains in the north-east: ridges, forests and snowfields.
    let range = |p: &Painter, x: i32, y: i32| {
        x >= 226 + p.wob(x, y, 14, 3208, 6) && (48..=108).contains(&(y + p.wob(x, y, 12, 3209, 5)))
    };
    p.fill(
        |p, x, y| range(p, x, y) && !p.is_water(x, y),
        |p, x, y| {
            let n = p.noise(x, y, 9, 3210);
            if n > 480 {
                p.biome(x, y).cliff
            } else if n > 380 {
                p.biome(x, y).forest
            } else {
                p.biome(x, y).plain
            }
        },
    );
    // Cliffs where the mountains and the peninsula's spine meet the sea, broken by coves.
    let cliffs = |p: &Painter, x: i32, y: i32| {
        let (kx, ky) = canonical(p, x, y);
        let spine = (100..=154).contains(&kx) && (82..=104).contains(&ky);
        let mountains = kx >= 180 && (40..=116).contains(&ky);
        spine || mountains
    };
    p.paint(
        |p, x, y| {
            !p.is_water(x, y)
                && cliffs(p, x, y)
                && p.noise(x, y, 7, 3211) > 380
                && (-4..=4).any(|dy| (-4..=4).any(|dx| p.is_water(x + dx, y + dy)))
        },
        ROCK,
    );
    p.paint(
        |p, x, y| {
            let (kx, ky) = canonical(p, x, y);
            !p.is_water(x, y)
                && (112..=142).contains(&kx)
                && (82..=88).contains(&ky)
                && p.noise(x, y, 6, 3212) > 470
        },
        ROCK,
    );
    // The rivers: one through a delta of reeds pushed out into the sea in the west, one into
    // the gulf in the north-east.
    p.blob(46, 90, 13, 4, 3213, |p, x, y| {
        if p.is_water(x, y) || p.is_plain(p.get(x, y)) {
            MARSH
        } else {
            p.get(x, y)
        }
    });
    p.stroke(&[(0, 64), (18, 70), (30, 72), (38, 82)], 1, WATER);
    for points in [
        &[(38, 82), (48, 78), (58, 80)][..],
        &[(38, 82), (48, 90), (60, 94)][..],
        &[(38, 82), (42, 96), (50, 104)][..],
    ] {
        p.stroke(points, 1, WATER);
    }
    p.paint(
        |p, x, y| p.get(x, y) == MARSH && (490..520).contains(&p.noise(x, y, 6, 3214)),
        WATER,
    );
    p.stroke(&[(198, 0), (194, 20), (198, 36), (192, 50)], 1, WATER);
    // Port towns on the bays, the fortress on the island, villages.
    for (x, y, r, s) in [
        (76, 68, 9, 3216),
        (206, 100, 8, 3217),
        (114, 78, 5, 3218),
        (cx, cy, 7, 3219),
    ] {
        p.city(
            |p, xx, yy| {
                let (mx, my) = p.mirror_of(x, y);
                [(x, y), (mx, my)]
                    .iter()
                    .any(|&(tx, ty)| (xx - tx).pow(2) + (yy - ty).pow(2) <= r * r)
            },
            x,
            y,
            r / 2,
            5,
            s,
        );
    }
    for (x, y) in [(56, 40), (130, 40), (200, 34), (40, 96), (210, 120)] {
        p.village(x, y, 4, 3220);
    }
    p.landmarks(22, 18, 3221, |p, x, y| basin(p, x, y) > 200);
    // The coast road round the sea, the roads to the bases, and the bridges to the island.
    p.fill(
        |p, x, y| (-30..-24).contains(&offshore(p, x, y)),
        |p, x, y| p.road_tile(x, y),
    );
    p.road_y(cx - 1, 30, 92, 2);
    p.road_y(cx - 3, 92, cy, 6);
    p.road_y(25, 26, 48, 2);
    p.road_y(97, 22, 46, 2);
    p.road_y(161, 24, 46, 2);
    p.road_y(227, 32, 50, 2);
    // Ore: round the ring between the bases, on the peninsula and the island.
    for (x, y, r, s) in [
        (cx, 122, 5, 3222),
        (127, 74, 4, 3223),
        (60, 30, 4, 3224),
        (128, 30, 4, 3225),
        (194, 30, 4, 3226),
        (22, 76, 5, 3227),
        (204, 120, 4, 3228),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 3229);
    p.finish(2, 3230);
}

pub const MANGROVE_COAST: Layout = Layout {
    width: 256,
    height: 224,
    homes: &[(86, 22), (164, 30), (88, 84), (162, 78)],
};

pub fn mangrove_coast_biome(_: &Painter, _: i32, _: i32) -> Biome {
    TROPIC
}

/// Mangrove Coast, 256 x 224: a tropical coast of tides. The sea lies along the west and east
/// edges; inland from it a belt of mangrove forest and tidal mudflats is cut by winding creeks,
/// with shell ridges, sand spits and fishing villages on stilts where the creeks meet the sea.
/// The bases stand on the firm upland in the middle, open grass and jungle with rice paddies,
/// plantations and a market town. Armies cross the upland freely; the mud belts are slow
/// flanks without building ground, and their rich ore lies out among the mangroves.
pub fn mangrove_coast(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    let (cx, cy) = (w / 2, h / 2);
    let edge = |p: &Painter, x: i32| x.min(p.w - 1 - x);
    // The sea, and the tidal belt between it and the upland: mud and mangroves.
    let belt = |p: &Painter, x: i32, y: i32| {
        edge(p, x) < 54 + p.wob(x, y, 30, 3300, 9) + p.wob(x, y, 10, 3301, 3)
    };
    p.paint(belt, MARSH);
    p.paint(
        |p, x, y| edge(p, x) < 16 + p.wob(x, y, 30, 3302, 6) + p.wob(x, y, 8, 3303, 1),
        WATER,
    );
    p.paint(
        |p, x, y| {
            let n = p.noise(x, y, 9, 3304);
            p.get(x, y) == MARSH && (n > 540 || (edge(p, x) < 26 && n > 420))
        },
        MANGROVE,
    );
    // Tidal creeks winding in from the sea, branching as they go, and the small creeks that
    // wander through the mud between them.
    p.paint(
        |p, x, y| {
            [MARSH, MANGROVE].contains(&p.get(x, y))
                && (494..522).contains(&p.noise(x, y, 14, 3325))
                && p.noise(x, y, 26, 3326) > 430
        },
        WATER,
    );
    for (points, r) in [
        (
            &[
                (0, 40),
                (10, 43),
                (18, 42),
                (26, 37),
                (33, 40),
                (40, 46),
                (47, 44),
                (54, 43),
                (60, 47),
                (64, 52),
            ][..],
            2,
        ),
        (
            &[(26, 37), (30, 30), (34, 24), (40, 19), (44, 13), (50, 9)][..],
            1,
        ),
        (&[(40, 46), (41, 54), (45, 60), (50, 64), (54, 70)][..], 1),
        (
            &[
                (0, 92),
                (9, 88),
                (17, 86),
                (24, 90),
                (30, 94),
                (37, 91),
                (43, 88),
                (50, 92),
                (57, 97),
                (64, 95),
            ][..],
            2,
        ),
        (&[(30, 94), (32, 100), (37, 104), (46, 106)][..], 1),
        (
            &[
                (255, 20),
                (246, 22),
                (239, 26),
                (232, 23),
                (226, 19),
                (219, 24),
                (213, 30),
                (206, 28),
                (198, 25),
            ][..],
            2,
        ),
        (&[(226, 19), (225, 12), (221, 7), (214, 5), (208, 2)][..], 1),
        (
            &[
                (255, 70),
                (247, 66),
                (241, 65),
                (235, 70),
                (229, 74),
                (223, 70),
                (217, 66),
                (211, 70),
                (205, 76),
                (198, 74),
                (192, 70),
            ][..],
            2,
        ),
        (
            &[(229, 74), (233, 82), (235, 90), (231, 98), (227, 105)][..],
            1,
        ),
    ] {
        p.stroke(points, r, WATER);
    }
    // Shell ridges along the old shorelines, and sand spits out into the sea.
    for points in [
        &[(22, 2), (18, 20), (22, 30)][..],
        &[(20, 58), (24, 74), (20, 84)][..],
        &[(232, 36), (236, 50), (232, 58)][..],
        &[(14, 58), (6, 70)][..],
        &[(242, 80), (250, 94)][..],
    ] {
        p.stroke_with(points, 1, |p, x, y| {
            if p.is_water(x, y) && edge(p, x) > 12 {
                WATER
            } else {
                SAND
            }
        });
    }
    p.paint(
        |p, x, y| p.get(x, y) == SAND && p.noise(x, y, 4, 3305) > 560,
        PALMS,
    );
    // The upland: grass and jungle, rocky knolls, rice paddies and plantations.
    let upland = move |x: i32| (x - cx).abs() < 66;
    p.forests(15, 3306, 600, move |x, _| upland(x));
    p.paint(
        |p, x, y| p.get(x, y) == GRASS && p.noise(x, y, 7, 3307) > 790,
        ROCK,
    );
    for (x, y, r, s) in [(112, 48, 8, 3308), (140, 100, 7, 3309), (70, 60, 6, 3310)] {
        p.patch(x, y, r, 3, s, CROPS);
    }
    for (x, y, r, s) in [(196, 104, 5, 3311), (120, 6, 5, 3312)] {
        p.patch(x, y, r, 2, s, ORCHARD);
    }
    // Fishing villages on stilts over the creek mouths.
    for (x, y) in [(18, 46), (18, 88), (240, 24), (242, 66)] {
        p.fill(
            |p, xx, yy| {
                let (kx, ky) = canonical(p, xx, yy);
                let (mx, my) = p.mirror_of(x, y);
                [(x, y), (mx, my)]
                    .iter()
                    .any(|&(tx, ty)| (xx - tx).pow(2) + (yy - ty).pow(2) <= 42)
                    && [MARSH, MANGROVE, SAND].contains(&p.get(xx, yy))
                    && hash(kx, ky, 3313).is_multiple_of(2)
            },
            |_, _, _| COTTAGE,
        );
    }
    // The market town and the upland villages.
    p.city(
        |p, x, y| centre_d2(p, x, y) <= (13 + p.wob(x, y, 8, 3314, 3)).pow(2),
        cx,
        cy,
        5,
        5,
        3315,
    );
    for (x, y) in [(112, 50), (140, 98), (72, 58), (128, 14)] {
        p.village(x, y, 4, 3316);
    }
    p.landmarks(20, 18, 3317, move |_, x, _| upland(x));
    // Roads along the upland and causeways out over the mud to the fishing villages.
    p.road_y(cx - 1, 8, cy, 2);
    p.road_x(54, 18, cx, 2);
    p.road_x(cy - 1, 70, w - 71, 2);
    p.road_y(87, 30, 54, 2);
    p.road_y(165, 38, 54, 2);
    p.road_x(24, 165, 240, 2);
    // Clear ways through the upland woods between the bases.
    let clear = |p: &Painter, x: i32, y: i32| {
        let t = p.get(x, y);
        if t == FOREST || t == ROCK { GRASS } else { t }
    };
    p.stroke_with(&[(86, 30), (90, 84), (96, 140)], 4, clear);
    p.stroke_with(&[(90, 30), (164, 34)], 4, clear);
    p.stroke_with(&[(90, 84), (162, 80)], 4, clear);
    // Ore: rich fields out in the mud, fields on the upland.
    for (x, y, r, s) in [
        (36, 68, 5, 3318),
        (220, 46, 5, 3319),
        (128, 60, 4, 3320),
        (124, 96, 4, 3321),
        (66, 104, 4, 3322),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 3323);
    p.finish(1, 3324);
}

pub const COASTAL_CLIFFS: Layout = Layout {
    width: 240,
    height: 240,
    homes: &[(22, 22), (84, 20), (20, 86), (86, 86), (140, 48)],
};

pub fn coastal_cliffs_biome(_: &Painter, _: i32, _: i32) -> Biome {
    TEMPERATE
}

/// Coastal Cliffs, 240 x 240: a high granite coast. The sea fills the north-east and south-west
/// corners and the land meets it in long lines of cliffs, broken only at a few sandy coves, with
/// sea stacks standing off them. Above lies a moorland plateau of grass and heather, granite
/// tors and bogs, with small fields walled by hedges round the stone hamlets. A lighthouse town
/// stands on a headland, and a wooded river valley cuts down through the cliffs to a harbour.
pub fn coastal_cliffs(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    let (cx, cy) = (w / 2, h / 2);
    // How far (cells along a row) a cell lies inland of the cliff line; the sea where negative.
    let inland = |p: &Painter, x: i32, y: i32| {
        130 + p.wob(x, y, 50, 3400, 22) + p.wob(x, y, 14, 3401, 7) + p.wob(x, y, 5, 3426, 2)
            - (x - y).abs()
    };
    p.paint(|p, x, y| inland(p, x, y) < 0, WATER);
    // The headland of the lighthouse.
    p.blob(188, 36, 11, 3, 3402, |_, _, _| GRASS);
    p.stroke(&[(180, 44), (196, 28)], 4, GRASS);
    // The moor: heather, bogs and granite tors in the high middle of the land; small fields
    // walled with hedges towards the coasts and round the bases.
    let moor = |p: &Painter, x: i32, y: i32| (x - y).abs() < 64 + p.wob(x, y, 24, 3403, 10);
    p.paint(
        |p, x, y| moor(p, x, y) && p.get(x, y) == GRASS && p.noise(x, y, 9, 3404) > 560,
        DIRT,
    );
    p.paint(
        |p, x, y| {
            moor(p, x, y)
                && p.get(x, y) == DIRT
                && p.noise(x, y, 5, 3405) > 700
                && !p.near_start(x, y, 16)
        },
        MARSH,
    );
    p.paint(
        |p, x, y| {
            moor(p, x, y)
                && p.is_plain(p.get(x, y))
                && p.noise(x, y, 4, 3406) > 790
                && !p.near_start(x, y, 12)
        },
        ROCK,
    );
    let field = |p: &Painter, x: i32, y: i32| {
        let (u, v) = p.centred(x, y);
        let (u, v) = (
            u + 2 * p.wob(x, y, 12, 3427, 2),
            v + 2 * p.wob(x, y, 12, 3428, 2),
        );
        ((u + 9).div_euclid(18), (v + 7).div_euclid(14))
    };
    p.fill(
        |p, x, y| !moor(p, x, y) && p.get(x, y) == GRASS,
        |p, x, y| {
            let (bu, bv) = field(p, x, y);
            let key = if (bv, bu) < (-bv, -bu) {
                (bu, bv)
            } else {
                (-bu, -bv)
            };
            match hash(key.0, key.1, 3407) % 10 {
                0 | 1 => CROPS,
                2 => DIRT,
                _ => GRASS,
            }
        },
    );
    p.paint(
        |p, x, y| {
            let (bu, bv) = field(p, x, y);
            let key = if (bv, bu) < (-bv, -bu) {
                (bu, bv)
            } else {
                (-bu, -bv)
            };
            let west = field(p, x - 1, y).0 != bu && !hash(key.0, key.1, 3408).is_multiple_of(3);
            let north = field(p, x, y - 1).1 != bv && !hash(key.0, key.1, 3409).is_multiple_of(3);
            !moor(p, x, y)
                && p.is_plain(p.get(x, y))
                && inland(p, x, y) > 6
                && !p.near_start(x, y, 6)
                && ((west && (y / 2) % 5 != 0) || (north && (x / 2) % 5 != 0))
        },
        ORCHARD,
    );
    // The river valley: wooded slopes down to the harbour.
    p.stroke(
        &[
            (126, 100),
            (134, 94),
            (138, 88),
            (146, 86),
            (154, 80),
            (160, 82),
            (168, 78),
            (174, 72),
            (182, 72),
            (190, 70),
            (198, 68),
            (214, 64),
        ],
        1,
        WATER,
    );
    p.paint(
        |p, x, y| {
            p.is_plain(p.get(x, y))
                && (-4..=4).any(|d| p.is_water(x + d, y) || p.is_water(x, y + d))
                && inland(p, x, y) > 10
                && p.noise(x, y, 6, 3410) > 470
        },
        FOREST,
    );
    p.blob(206, 70, 5, 2, 3411, |_, _, _| WATER);
    // The cliffs, broken by a few coves with their beaches; sea stacks off them.
    p.paint(
        |p, x, y| {
            !p.is_water(x, y)
                && inland(p, x, y) < 11 + p.wob(x, y, 5, 3412, 4)
                && p.noise(x, y, 6, 3413) > 260
        },
        ROCK,
    );
    p.paint(
        |p, x, y| {
            p.is_water(x, y)
                && (-16..=-3).contains(&inland(p, x, y))
                && p.noise(x, y, 3, 3414) > 740
        },
        ROCK,
    );
    for points in [
        &[(140, 30), (162, 8)][..],
        &[(166, 62), (184, 44)][..],
        &[(210, 112), (236, 86)][..],
    ] {
        p.stroke_with(points, 4, |p, x, y| match p.get(x, y) {
            ROCK => SAND,
            tile => tile,
        });
    }
    // The lighthouse town, the harbour town, and the stone hamlets of the moor.
    for (x, y, r, s) in [(188, 36, 7, 3415), (204, 74, 8, 3416)] {
        p.city(
            |p, xx, yy| {
                let (mx, my) = p.mirror_of(x, y);
                [(x, y), (mx, my)]
                    .iter()
                    .any(|&(tx, ty)| (xx - tx).pow(2) + (yy - ty).pow(2) <= r * r)
            },
            x,
            y,
            2,
            5,
            s,
        );
    }
    p.stroke(&[(195, 29), (195, 29)], 1, PLAZA);
    p.stroke(&[(195, 29), (195, 29)], 0, TOWER);
    for (x, y) in [(56, 54), (112, 40), (60, 110), (150, 112), (118, 70)] {
        p.village(x, y, 4, 3417);
    }
    p.landmarks(20, 16, 3418, |p, x, y| inland(p, x, y) > 12);
    // Roads: the cliff road, the road over the moor, lanes to the bases and the towns.
    let lane = |p: &Painter, x: i32, y: i32| p.road_tile(x, y);
    p.stroke_with(
        &[(118, 4), (150, 30), (176, 52), (196, 68), (226, 104)],
        1,
        lane,
    );
    p.stroke_with(&[(24, 24), (60, 56), (88, 88), (cx, cy)], 1, lane);
    p.stroke_with(&[(85, 22), (100, 40), (142, 50)], 1, lane);
    p.stroke_with(&[(22, 88), (60, 110), (88, 88)], 1, lane);
    p.stroke_with(&[(142, 50), (176, 52), (188, 40)], 1, lane);
    // Ore: on the moor in the middle, by the headland and out on the flanks.
    for (x, y, r, s) in [
        (cx, 112, 5, 3419),
        (160, 66, 4, 3420),
        (60, 130, 4, 3421),
        (52, 28, 4, 3422),
        (124, 22, 4, 3423),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 3424);
    p.finish(2, 3425);
}
