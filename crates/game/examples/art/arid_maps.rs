//! Maps of the dry country: a sand sea of long parallel dunes and the corridors between them
//! (`dune_sea`), eroded clay badlands with a dry river and a ghost mining town (`badlands`), an oil
//! coast of pipelines, tank farms and salt marsh between two gulf shores (`oil_fields`), and a
//! green river valley winding between two desert plateaus (`nile_valley`). Sand, marsh and the
//! sabkha are ground armies cross but nobody builds on; dune crests, buttes and cliffs are mesa.
//!
//! Seeds 2000 to 2999 are this file's.
use super::*;

/// A displacement of up to about `amp` cells that is odd about the map's centre: the mirror
/// cell is displaced the other way, so a band laid along `offset + odd(..)` is its own mirror.
fn odd(p: &Painter, x: i32, y: i32, scale: i32, seed: u32, amp: i32) -> i32 {
    let (mx, my) = p.mirror_of(x, y);
    let d = fbm(x, y, scale, seed) - fbm(mx, my, scale, seed);
    (d * i64::from(amp) / 384).clamp(-i64::from(amp), i64::from(amp)) as i32
}

/// A palm oasis around (x, y) and its mirror: a pool of radius `r` ringed by palms, in a patch of
/// watered earth.
fn oasis(p: &mut Painter, x: i32, y: i32, r: i32, seed: u32) {
    p.blob(x, y, r + 6, 2, seed, |p, x, y| {
        if p.is_plain(p.get(x, y)) {
            DIRT
        } else {
            p.get(x, y)
        }
    });
    p.blob(x, y, r + 3, 1, seed + 1, |p, x, y| {
        if p.get(x, y) == DIRT {
            PALMS
        } else {
            p.get(x, y)
        }
    });
    p.blob(x, y, r, 1, seed + 2, |_, _, _| WATER);
}

// ---------------------------------------------------------------------------------------------
// Dune Sea.

/// Cells between the axes of neighbouring interdune corridors.
const DUNE_SPACING: i32 = 48;

pub const DUNE_SEA: Layout = Layout {
    width: 256,
    height: 256,
    homes: &[(222, 25), (29, 35), (29, 83), (29, 131), (29, 179)],
};

/// Cells across the dunes from the axis of the middle corridor: odd about the centre, so the
/// corridors and the dunes between them are their own mirrors.
fn dune_offset(p: &Painter, x: i32, y: i32) -> i32 {
    let (u, v) = p.centred(x, y);
    (v - u / 8) / 2 + p.meander(x, p.w, 52, 2002, 7) + odd(p, x, y, 36, 2003, 4)
}

/// The row of the axis of corridor `j` at column `x`, near enough. The corridors lie between
/// the dunes, and a dune runs through the centre: corridor `j` is the one whose axis is
/// `DUNE_SPACING * j + DUNE_SPACING / 2` cells across the dunes from it.
fn corridor_y(p: &Painter, j: i32, x: i32) -> i32 {
    let (u, _) = p.centred(x, 0);
    let axis = DUNE_SPACING * j + DUNE_SPACING / 2;
    let v = 2 * (axis - p.meander(x, p.w, 52, 2002, 7)) + u / 8;
    (v + p.h - 1) / 2
}

/// Cells from the nearest dune crest: 0 on a crest, `DUNE_SPACING / 2` on a corridor's axis.
fn from_crest(p: &Painter, x: i32, y: i32) -> i32 {
    let q = dune_offset(p, x, y).rem_euclid(DUNE_SPACING);
    q.min(DUNE_SPACING - q)
}

/// Saddles in the crests: four on every dune, where the sand is low enough to drive over.
fn saddle(p: &Painter, x: i32, y: i32) -> bool {
    let k = (dune_offset(p, x, y) + DUNE_SPACING / 2).div_euclid(DUNE_SPACING);
    // The dune and the place along it, alike for a cell and its mirror.
    let (k, t) = match k {
        1.. => (k, x),
        0 => (0, x.min(p.w - 1 - x)),
        _ => (-k, p.w - 1 - x),
    };
    (t + (hash(k, 0, 2006) % 64) as i32).rem_euclid(64) < 18 || p.noise(x, y, 9, 2007) > 690
}

pub fn dune_sea_biome(_: &Painter, _: i32, _: i32) -> Biome {
    DESERT_BIOME
}

/// Dune Sea, 256 x 256: a great sand sea. Long parallel dunes run across the map from west to
/// east, a belt of loose sand under a knife-edge crest that tanks cannot climb except at the
/// saddles; between them lie firm, flat interdune corridors where bases stand and armies move
/// fast. Caravan towns stand at the oases, a desert highway crosses the dunes from north to south,
/// and a salt pan in the middle is the one place where the dunes give out.
pub fn dune_sea(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    let (cx, cy) = (w / 2, h / 2);
    // The dunes: broad belts of loose sand under sharp crests, broken by saddles, and here and
    // there a star dune heaped on the belt.
    p.paint(
        |p, x, y| from_crest(p, x, y) <= 8 + p.wob(x, y, 12, 2004, 3),
        SAND,
    );
    p.paint(
        |p, x, y| from_crest(p, x, y) <= 2 + p.wob(x, y, 8, 2005, 1) && !saddle(p, x, y),
        MESA,
    );
    p.paint(
        |p, x, y| p.get(x, y) == SAND && p.noise(x, y, 4, 2008) > 800,
        MESA,
    );
    // The floors of the corridors: long flats of gravel and old lake marl along their axes.
    p.paint(
        |p, x, y| {
            p.get(x, y) == DESERT
                && from_crest(p, x, y) >= 17 + p.wob(x, y, 10, 2009, 3)
                && p.noise(x, y, 16, 2010) > 440
        },
        DIRT,
    );
    // The salt pan in the middle, where the dunes give out.
    p.paint(
        |p, x, y| {
            let (u, v) = p.centred(x, y);
            u * u / 9 + v * v / 4 <= (26 + p.wob(x, y, 14, 2011, 4)).pow(2)
        },
        DIRT,
    );
    p.paint(
        |p, x, y| {
            let (u, v) = p.centred(x, y);
            p.get(x, y) == SAND && u * u / 9 + v * v / 4 <= 36 * 36
        },
        DESERT,
    );
    // The caravan town at its oasis, and lone oases in the corridors.
    let town = (140, corridor_y(p, -2, 140));
    oasis(p, town.0 - 12, town.1 + 1, 3, 2012);
    p.city(
        |p, x, y| {
            let d = |cx: i32, cy: i32| (x - cx).pow(2) + (y - cy).pow(2) <= 10 * 10;
            let (mx, my) = p.mirror_of(town.0 + 3, town.1);
            (d(town.0 + 3, town.1) || d(mx, my)) && ![PALMS, WATER].contains(&p.get(x, y))
        },
        town.0 + 3,
        town.1,
        4,
        5,
        2015,
    );
    for (x, j, r, s) in [(72, -1, 3, 2016), (90, 0, 2, 2019), (150, -3, 2, 2022)] {
        oasis(p, x, corridor_y(p, j, x), r, s);
    }
    p.village(town.0 + 20, corridor_y(p, -2, town.0 + 20), 3, 2026);
    p.landmarks(22, 16, 2025, |p, x, y| {
        from_crest(p, x, y) >= 12 && centre_d2(p, x, y) > 44 * 44
    });
    // The caravan route along the oasis corridor, and the highway over the dunes.
    let route: Vec<(i32, i32)> = (0..=13)
        .map(|k| {
            let x = 38 + 14 * k;
            (x, corridor_y(p, -2, x) + 6)
        })
        .collect();
    p.stroke_with(&route, 0, |p, x, y| p.road_tile(x, y));
    p.stroke_with(&[(146, 0), (139, 64), (cx, cy)], 1, |p, x, y| {
        p.road_tile(x, y)
    });
    // Ore: fields out in every corridor, and rich ones on the salt pan.
    for (x, j, r, s) in [
        (160, -3, 4, 2030),
        (96, -2, 5, 2031),
        (186, -2, 4, 2032),
        (104, -1, 4, 2033),
        (190, -1, 5, 2034),
    ] {
        p.ore(x, corridor_y(p, j, x), r, s);
    }
    p.ore(cx - 24, cy - 3, 5, 2035);
    p.ore(cx + 6, cy - 14, 4, 2036);
    p.home_ore(4, 2040);
    p.finish(1, 2041);
}

// ---------------------------------------------------------------------------------------------
// Badlands.

pub const BADLANDS: Layout = Layout {
    width: 224,
    height: 224,
    homes: &[(25, 25), (196, 25), (109, 18), (18, 109)],
};

/// Grey and brown clay country: the flats of the badlands where they are not drifted sand.
const BADLAND: Biome = Biome {
    plain: DIRT,
    ore: DESERT_ORE,
    cliff: MESA,
    forest: FOREST,
    house: ADOBE,
    road: TRACK,
    shore: SAND,
    block: DESERT_CITY,
    tower: DESERT_TOWER,
};

pub fn badlands_biome(p: &Painter, x: i32, y: i32) -> Biome {
    // Clay flats and drifted sand in great patches; juniper rather than palms in either.
    if p.noise(x, y, 34, 2111) > 430 {
        BADLAND
    } else {
        Biome {
            forest: FOREST,
            ..DESERT_BIOME
        }
    }
}

/// Badlands, 224 x 224: eroded clay country. A maze of dry washes winds through it between
/// buttes, hoodoos and banded ridges of harder rock; there are a hundred ways through and
/// none of them straight. A dry river crosses it from north to south, and an old railway from
/// west to east; where they meet stands a ghost mining town, with the head frames and tailings
/// of its mines in the hills around, and the ore with them.
pub fn badlands(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    let (cx, cy) = (w / 2, h / 2);
    // Washes: dry gullies of sand winding through the clay, with their side gullies.
    p.paint(
        |p, x, y| !p.near_start(x, y, 16) && (p.noise(x, y, 26, 2102) - 512).abs() <= 9,
        SAND,
    );
    p.paint(
        |p, x, y| {
            !p.near_start(x, y, 16)
                && (p.noise(x, y, 12, 2103) - 512).abs() <= 7
                && p.noise(x, y, 30, 2112) > 470
        },
        SAND,
    );
    // Banded ridges: the hard beds of the strata stand out as low walls, broken every few cells.
    p.paint(
        |p, x, y| {
            !p.near_start(x, y, 14)
                && (592..618).contains(&p.noise(x, y, 32, 2104))
                && p.noise(x, y, 9, 2108) > 480
        },
        MESA,
    );
    // The dry river: a bed of sand from the north edge to the south, between flats of silt
    // with cottonwoods and the last pools of the spring floods.
    let bed = |p: &Painter, x: i32, y: i32| {
        let (u, v) = p.centred(x, y);
        (u - v * 2 / 5 - 2 * p.meander(y, p.h, 44, 2109, 16)).abs()
    };
    p.paint(|p, x, y| bed(p, x, y) <= 9 + p.wob(x, y, 9, 2113, 2), SAND);
    p.paint(
        |p, x, y| {
            let d = bed(p, x, y);
            d > 9 && d <= 20 + p.wob(x, y, 11, 2114, 5) && p.get(x, y) != SAND
        },
        DIRT,
    );
    p.paint(
        |p, x, y| {
            let d = bed(p, x, y);
            (8..=22).contains(&d) && p.get(x, y) == DIRT && p.noise(x, y, 5, 2115) > 720
        },
        FOREST,
    );
    p.paint(
        |p, x, y| bed(p, x, y) <= 4 && p.noise(x, y, 6, 2116) > 790,
        WATER,
    );
    // Buttes and mesas: flat-topped rocks with aprons of scree, in clusters, out of the river
    // and the town; and a few great mesas standing alone.
    let rough = |p: &Painter, x: i32, y: i32| {
        !p.near_start(x, y, 16) && bed(p, x, y) > 24 && centre_d2(p, x, y) > 26 * 26
    };
    p.paint(
        |p, x, y| rough(p, x, y) && p.is_plain(p.get(x, y)) && p.noise(x, y, 10, 2105) > 640,
        SAND,
    );
    p.paint(
        |p, x, y| rough(p, x, y) && p.noise(x, y, 10, 2105) > 690,
        MESA,
    );
    for (x, y, r, s) in [(64, 58, 6, 2120), (150, 50, 5, 2123), (40, 150, 5, 2126)] {
        p.blob(x, y, r + 1, 3, s, |p, x, y| {
            if p.is_plain(p.get(x, y)) {
                SAND
            } else {
                p.get(x, y)
            }
        });
        p.blob(x, y, r, 3, s + 1, |_, _, _| MESA);
    }
    // Hoodoos: lone spires, thick in some fields of them.
    p.paint(
        |p, x, y| {
            let (kx, ky) = canonical(p, x, y);
            [DESERT, DIRT].contains(&p.get(x, y))
                && !p.near_start(x, y, 16)
                && p.noise(x, y, 20, 2117) > 560
                && hash(kx, ky, 2118).is_multiple_of(19)
        },
        MESA,
    );
    // Juniper in the draws.
    p.forests(9, 2119, 790, |_, _| true);
    // The ghost town where the river and the railway meet: a mining camp gone to rubble, with
    // the head frames and tailings of its mines in the hills around.
    let period = 5;
    p.city(
        |p, x, y| centre_d2(p, x, y) <= (15 + p.wob(x, y, 8, 2190, 2)).pow(2) && bed(p, x, y) > 8,
        cx,
        cy,
        6,
        period,
        2191,
    );
    p.fill(
        |p, x, y| {
            [DESERT_CITY, DESERT_TOWER, STREET].contains(&p.get(x, y))
                && centre_d2(p, x, y) <= 18 * 18
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
            let cell = hash(kx, ky, 2192) % 6;
            if p.get(x, y) == STREET {
                return if cell < 2 { DESERT } else { STREET };
            }
            match hash(key.0, key.1, 2193) % 10 {
                0..=3 => [PLAZA, ADOBE, DESERT, PLAZA, DESERT, DIRT][cell as usize],
                4 | 5 => [ADOBE, DESERT, DIRT, ADOBE, DESERT, DESERT][cell as usize],
                _ => p.get(x, y),
            }
        },
    );
    for (x, y, s) in [(86, 52, 2194), (160, 74, 2196), (52, 92, 2198)] {
        p.patch(x, y, 6, 2, s, DIRT);
        p.blob(x, y, 1, 0, s + 1, |_, _, _| DESERT_TOWER);
        p.village(x - 5, y - 5, 3, s);
    }
    // The railway: from the west edge to the ghost town and on to the east, through cuttings.
    let rail = |p: &Painter, x: i32, y: i32| {
        let (u, v) = p.centred(x, y);
        (v - u * 2 / 5 - 2 * p.meander(x, p.w, 64, 2110, 6)).abs()
    };
    p.paint(|p, x, y| rail(p, x, y) <= 2, TRACK);
    // Dirt roads between the bases and in to the town, winding round the rocks.
    for track in [
        &[(28, 27), (50, 35), (78, 30), (110, 21)][..],
        &[(27, 28), (35, 56), (27, 82), (20, 110)][..],
        &[(110, 21), (104, 48), (114, 80), (cx - 4, cy - 14)][..],
        &[(20, 110), (52, 104), (80, 114), (cx - 14, cy - 2)][..],
        &[(196, 27), (168, 35), (140, 26), (112, 20)][..],
        &[(198, 28), (190, 56), (200, 84), (204, 112)][..],
    ] {
        p.stroke_with(track, 1, |p, x, y| p.road_tile(x, y));
    }
    // Ore: rich fields at the mines and the town, fields along the river and out on the flanks.
    for (x, y, r, s) in [
        (92, 58, 5, 2130),
        (154, 80, 5, 2131),
        (58, 98, 5, 2132),
        (98, 98, 4, 2133),
        (140, 40, 4, 2134),
        (44, 64, 4, 2135),
        (178, 62, 4, 2136),
        (70, 26, 4, 2137),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 2140);
    p.finish(1, 2141);
}

// ---------------------------------------------------------------------------------------------
// Oil Fields.

pub const OIL_FIELDS: Layout = Layout {
    width: 256,
    height: 192,
    homes: &[(48, 22), (205, 22), (125, 16), (90, 64)],
};

pub fn oil_fields_biome(_: &Painter, _: i32, _: i32) -> Biome {
    DESERT_BIOME
}

/// Oil Fields, 256 x 192: oil country on a hot gulf coast. The sea lies along the west and
/// east, fringed by sabkha (salt marsh) and mangrove creeks, with a refinery port and its tank
/// farms on each shore; between them flat desert and salt flats to the horizon, crossed by
/// pipelines and their service roads in dead straight lines, with pump stations where they
/// cross and a gathering station at the centre. Open, industrial, long sight lines, and the ore
/// lies thick along the pipelines.
pub fn oil_fields(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    let (cx, cy) = (w / 2, h / 2);
    // The gulf along the west and east edges, and the creeks that reach inland from it.
    let inland = |p: &Painter, x: i32, y: i32| {
        x.min(p.w - 1 - x) - p.wob(x, y, 40, 2202, 10) - p.wob(x, y, 9, 2203, 3)
    };
    p.paint(|p, x, y| inland(p, x, y) < 19, WATER);
    // The ports stand on sand spits out in the gulf.
    p.blob(13, cy, 12, 3, 2208, |_, _, _| DESERT);
    p.stroke(&[(13, cy), (30, cy)], 7, DESERT);
    for creek in [
        &[(8, 50), (24, 54), (34, 48), (40, 50)][..],
        &[(8, 138), (22, 134), (30, 140)][..],
    ] {
        p.stroke(creek, 2, WATER);
    }
    // Sabkha: salt marsh along the shore and round the creeks, with mangroves at the water.
    p.paint(
        |p, x, y| {
            p.get(x, y) == DESERT
                && (inland(p, x, y) < 29 + p.wob(x, y, 10, 2204, 4) || p.noise(x, y, 7, 2205) > 760)
                && inland(p, x, y) < 42
                && p.centred(x, y).1.abs() > 25
                && p.noise(x, y, 12, 2206) > 360
        },
        MARSH,
    );
    p.paint(
        |p, x, y| {
            p.get(x, y) == MARSH
                && p.noise(x, y, 5, 2207) > 640
                && (-2..=2).any(|d| p.is_water(x + d, y) || p.is_water(x, y + d))
        },
        MANGROVE,
    );
    // Salt flats and gravel inland, a few low jebels, and wadis running down to the sabkha.
    p.paint(
        |p, x, y| p.get(x, y) == DESERT && p.noise(x, y, 26, 2210) > 640,
        DIRT,
    );
    for (x, y, r, s) in [(140, 58, 3, 2211), (176, 72, 2, 2212)] {
        p.blob(x, y, r + 2, 2, s, |p, x, y| {
            if p.is_plain(p.get(x, y)) {
                SAND
            } else {
                p.get(x, y)
            }
        });
        p.blob(x, y, r, 2, s + 1, |_, _, _| MESA);
    }
    for wadi in [
        &[(112, 72), (92, 80), (70, 76), (44, 84), (22, 84)][..],
        &[(164, 46), (186, 50), (212, 60), (240, 58)][..],
    ] {
        p.stroke_with(
            wadi,
            1,
            |p, x, y| {
                if p.is_water(x, y) { WATER } else { SAND }
            },
        );
    }
    // The refinery ports: a town of blocks and towers on each shore with its pier and its tank
    // farm.
    let port = (26, cy);
    p.city(
        |p, x, y| {
            let d = |cx: i32, cy: i32| (x - cx).pow(2) + (y - cy).pow(2) <= 14 * 14;
            let (mx, my) = p.mirror_of(port.0, port.1);
            d(port.0, port.1) || d(mx, my)
        },
        port.0,
        port.1,
        7,
        5,
        2215,
    );
    // Tank farms: round tanks in rows on a paved yard, laid out on the half of the map that
    // holds them so that the other is their mirror.
    let farm = |p: &Painter, x: i32, y: i32, (x0, y0, x1, y1): (i32, i32, i32, i32)| {
        let (kx, ky) = canonical(p, x, y);
        if !(x0..=x1).contains(&kx) || !(y0..=y1).contains(&ky) {
            None
        } else if (1..=2).contains(&(kx - x0).rem_euclid(4))
            && (1..=2).contains(&(ky - y0).rem_euclid(4))
        {
            Some(DESERT_TOWER)
        } else {
            Some(PLAZA)
        }
    };
    for rect in [(44, 66, 58, 80), (160, 64, 174, 74)] {
        p.fill(
            |p, x, y| farm(p, x, y, rect).is_some(),
            |p, x, y| farm(p, x, y, rect).unwrap_or(DESERT),
        );
    }
    // The gathering station at the centre, where the trunk lines meet.
    p.fill(
        |p, x, y| {
            let (u, v) = p.centred(x, y);
            u.abs() <= 25 && v.abs() <= 17
        },
        |p, x, y| {
            let (u, v) = p.centred(x, y);
            if [3, 5].contains(&u.rem_euclid(8)) && [3, 5].contains(&v.rem_euclid(8)) {
                DESERT_TOWER
            } else {
                PLAZA
            }
        },
    );
    // Pump stations where the pipelines cross, and the camps of the oil workers.
    for (x, y, s) in [
        (75, 43, 2220),
        (107, 43, 2221),
        (75, 99, 2222),
        (107, 99, 2223),
        (186, 43, 2224),
    ] {
        p.blob(x, y, 3, 1, s, |p, x, y| {
            if p.is_plain(p.get(x, y)) {
                PLAZA
            } else {
                p.get(x, y)
            }
        });
        for (dx, dy, tile) in [
            (0, 0, DESERT_TOWER),
            (1, 0, DESERT_TOWER),
            (-1, 1, DESERT_CITY),
            (1, -2, DESERT_CITY),
        ] {
            for (cx, cy) in [(x + dx, y + dy), p.mirror_of(x + dx, y + dy)] {
                if p.inside(cx, cy) && p.get(cx, cy) == PLAZA {
                    p.set(cx, cy, tile);
                }
            }
        }
    }
    for (x, y) in [(56, 118), (170, 34), (90, 150)] {
        p.village(x, y, 4, 2225);
    }
    p.landmarks(24, 16, 2226, |p, x, y| {
        inland(p, x, y) > 40 && centre_d2(p, x, y) > 30 * 30
    });
    // The pipelines and their service roads, dead straight across the desert, the coast highway
    // from port to port, and the piers.
    p.road_x(cy - 1, port.0, w - 1 - port.0, 2);
    p.road_x(40, 30, w - 31, 1);
    p.road_y(72, 10, h - 11, 1);
    p.road_y(104, 10, h - 11, 1);
    p.road_y(cx - 1, 10, cy, 2);
    p.road_y(8, 62, cy - 8, 2);
    // Ore: abundant along the pipelines, rich where they cross.
    for (x, y, r, s) in [
        (78, 50, 4, 2230),
        (98, 32, 4, 2231),
        (128, 56, 5, 2232),
        (66, 122, 4, 2233),
        (112, 112, 5, 2234),
        (150, 30, 4, 2235),
        (186, 62, 4, 2236),
        (30, 66, 4, 2237),
        (88, 88, 4, 2238),
        (160, 86, 4, 2239),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 2240);
    p.finish(1, 2241);
}

// ---------------------------------------------------------------------------------------------
// Nile Valley.

pub const NILE_VALLEY: Layout = Layout {
    width: 256,
    height: 256,
    homes: &[(29, 29), (224, 29), (22, 125), (52, 77), (52, 176)],
};

/// The green of the valley floor: clover and fields to the water, palms, mud-brick villages.
const FLOODPLAIN: Biome = Biome {
    plain: GRASS,
    ore: ORE,
    cliff: MESA,
    forest: PALMS,
    house: ADOBE,
    road: ROAD,
    shore: GRASS,
    block: DESERT_CITY,
    tower: DESERT_TOWER,
};

/// The column of the river's axis at row `y`. The river winds, and is its own mirror.
fn nile_x(p: &Painter, y: i32) -> i32 {
    (2 * p.meander(y, p.h, 70, 2302, 26) + p.w - 1) / 2
}

/// Cells from the river's axis.
fn nile_d(p: &Painter, x: i32, y: i32) -> i32 {
    let (u, _) = p.centred(x, y);
    (u - 2 * p.meander(y, p.h, 70, 2302, 26)).abs() / 2
}

/// How far the green reaches from the river's axis, here.
fn green_edge(p: &Painter, x: i32, y: i32) -> i32 {
    21 + p.wob(x, y, 18, 2303, 5)
}

pub fn nile_valley_biome(p: &Painter, x: i32, y: i32) -> Biome {
    if nile_d(p, x, y) <= green_edge(p, x, y) {
        FLOODPLAIN
    } else {
        DESERT_BIOME
    }
}

/// Nile Valley, 256 x 256: a great river winding through the desert from north to south. A
/// narrow green floodplain of fields, palm groves and villages hugs it, with a city on both
/// banks at the centre and the ruins of a temple; cliffs step up from the green to the open
/// desert plateaus where the bases lie. Granite cataracts cross the river with a ford below
/// each, two more fords and the city's bridges make the crossings, and the rich ground of the
/// valley is the prize between the two deserts.
pub fn nile_valley(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    let (cx, cy) = (w / 2, h / 2);
    // The river, with islands here and there and papyrus along its edges.
    p.paint(
        |p, x, y| nile_d(p, x, y) <= 4 + p.wob(x, y, 10, 2304, 1),
        WATER,
    );
    p.paint(
        |p, x, y| p.is_water(x, y) && nile_d(p, x, y) <= 1 && p.noise(x, y, 8, 2305) > 770,
        GRASS,
    );
    p.paint(
        |p, x, y| !p.is_water(x, y) && nile_d(p, x, y) <= 7 && p.noise(x, y, 6, 2306) > 610,
        MARSH,
    );
    // The fields of the floodplain: a patchwork of crops, fallow and clover, and palm groves.
    let in_green = |p: &Painter, x: i32, y: i32| nile_d(p, x, y) <= green_edge(p, x, y);
    p.fill(
        |p, x, y| p.get(x, y) == GRASS && nile_d(p, x, y) >= 8 && in_green(p, x, y),
        |p, x, y| {
            let (u, v) = p.centred(x, y);
            let (bu, bv) = ((u + 10).div_euclid(20), (v + 7).div_euclid(14));
            let key = if (bv, bu) < (-bv, -bu) {
                (bu, bv)
            } else {
                (-bu, -bv)
            };
            match hash(key.0, key.1, 2307) % 10 {
                0..=4 => CROPS,
                5 | 6 => DIRT,
                _ => GRASS,
            }
        },
    );
    p.paint(
        |p, x, y| {
            [GRASS, DIRT].contains(&p.get(x, y))
                && in_green(p, x, y)
                && p.noise(x, y, 9, 2308) > 640
        },
        PALMS,
    );
    // The desert's edge: a fringe of sand where the fields give out under the cliffs.
    p.paint(
        |p, x, y| {
            let e = green_edge(p, x, y);
            (e - 2..=e).contains(&nile_d(p, x, y)) && p.is_plain(p.get(x, y))
        },
        SAND,
    );
    // The escarpment: the cliffs of the plateau above the green, cut by wadis, and a second,
    // broken step further out.
    let wadi = |p: &Painter, x: i32, y: i32| {
        let (u, _) = p.centred(x, y);
        let west = u < 2 * p.meander(y, p.h, 70, 2302, 26);
        let t = if west { y } else { p.h - 1 - y };
        (t + 9).rem_euclid(44) < 16
    };
    p.paint(
        |p, x, y| {
            let (d, e) = (nile_d(p, x, y), green_edge(p, x, y));
            d > e && d <= e + 5 + p.wob(x, y, 6, 2309, 2) && !wadi(p, x, y)
        },
        MESA,
    );
    p.paint(
        |p, x, y| {
            (54..=56).contains(&(nile_d(p, x, y) + p.wob(x, y, 14, 2310, 4)))
                && p.noise(x, y, 10, 2311) > 520
                && !p.near_start(x, y, 16)
        },
        MESA,
    );
    // The open desert of the plateaus: gravel plains, fields of dunes, and the dry wadis that
    // run down to the gaps in the cliffs.
    p.paint(
        |p, x, y| p.get(x, y) == DESERT && p.noise(x, y, 22, 2317) > 690,
        DIRT,
    );
    p.paint(
        |p, x, y| p.get(x, y) == DESERT && !p.near_start(x, y, 18) && p.noise(x, y, 16, 2318) > 680,
        SAND,
    );
    for wadi in [
        &[(4, 64), (30, 58), (60, 50), (nile_x(p, 43) - 22, 43)][..],
        &[(6, 150), (34, 140), (70, 134), (nile_x(p, 131) - 22, 131)][..],
    ] {
        p.stroke_with(
            wadi,
            1,
            |p, x, y| {
                if p.get(x, y) == MESA { DESERT } else { SAND }
            },
        );
    }
    // The pyramids on the desert's edge above the city, and the causeway down to the valley.
    let giza = nile_x(p, 112) - 38;
    for (x, y, r) in [(giza, 108, 4), (giza - 8, 117, 3), (giza + 6, 119, 2)] {
        p.fill(
            |p, xx, yy| {
                let (kx, ky) = canonical(p, xx, yy);
                (kx - x).abs() <= r && (ky - y).abs() <= r
            },
            |_, _, _| MESA,
        );
    }
    p.stroke(&[(giza + 5, 108), (nile_x(p, 108) - 18, 108)], 1, PLAZA);
    // The cataracts: bands of granite across the valley, with rocks standing in the rapids.
    let cataract = 84;
    p.paint(
        |p, x, y| {
            let (_, v) = p.centred(x, y);
            let (_, vc) = p.centred(0, cataract);
            (v.abs() - vc.abs() + p.wob(x, y, 9, 2312, 4)).abs() <= 8
                && nile_d(p, x, y) <= green_edge(p, x, y)
                && p.noise(x, y, 4, 2313) > 590
        },
        ROCK,
    );
    // The city on both banks at the centre, with the ruins of a temple on the west bank.
    p.city(
        |p, x, y| {
            centre_d2(p, x, y) <= (24 + p.wob(x, y, 10, 2314, 3)).pow(2)
                && nile_d(p, x, y) <= green_edge(p, x, y) - 3
                && nile_d(p, x, y) > 5
        },
        cx,
        cy,
        9,
        5,
        2315,
    );
    let tx = nile_x(p, cy - 14) - 20;
    let temple = (tx - 8, cy - 26, tx + 8, cy - 10);
    let ruin = |p: &Painter, x: i32, y: i32| {
        let (kx, ky) = canonical(p, x, y);
        let (x0, y0, x1, y1) = temple;
        if !(x0..=x1).contains(&kx) || !(y0..=y1).contains(&ky) {
            return None;
        }
        let fallen = hash(kx, ky, 2316).is_multiple_of(4);
        let wall = kx == x0 || kx == x1 || ky == y0 || ky == y1;
        let gate = (ky - (y0 + y1) / 2).abs() <= 1;
        Some(if wall && !gate {
            if fallen { DESERT } else { DESERT_CITY }
        } else if (kx - x0) % 3 == 0 && (ky - y0) % 3 == 0 && !gate {
            if fallen { PLAZA } else { DESERT_TOWER }
        } else if fallen {
            DESERT
        } else {
            PLAZA
        })
    };
    p.fill(
        |p, x, y| ruin(p, x, y).is_some(),
        |p, x, y| ruin(p, x, y).unwrap_or(PLAZA),
    );
    // Villages along both banks.
    for (y, side, s) in [
        (20, -1, 2320),
        (52, 1, 2321),
        (70, -1, 2322),
        (100, 1, 2323),
        (110, -1, 2324),
    ] {
        p.village(nile_x(p, y) + side * 13, y, 4, s);
    }
    p.landmarks(22, 16, 2325, |p, x, y| {
        nile_d(p, x, y) > green_edge(p, x, y) + 10
    });
    // Fords where the river runs wide over gravel bars, one below each cataract.
    for yf in [34, cataract + 8] {
        let xf = nile_x(p, yf);
        p.stroke_with(&[(xf - 10, yf), (xf + 10, yf)], 7, |p, x, y| {
            match p.get(x, y) {
                WATER | MARSH | ROCK => SAND,
                tile => tile,
            }
        });
    }
    // Roads: along the west bank (the mirror is the east bank road), the highway over the river
    // through the city, a second bridge in the city, and roads down from the plateau.
    let bank: Vec<(i32, i32)> = (0..=16)
        .map(|k| {
            let y = (k * 16).min(h - 1);
            (nile_x(p, y) - 15, y)
        })
        .collect();
    p.stroke_with(&bank, 0, |p, x, y| p.road_tile(x, y));
    p.road_x(cy - 1, 40, w - 41, 2);
    p.road_x(cy - 13, cx - 34, cx + 34, 2);
    for road in [
        &[(31, 30), (60, 34), (nile_x(p, 34) - 15, 34)][..],
        &[(54, 79), (nile_x(p, 88) - 15, 88)][..],
    ] {
        p.stroke_with(road, 1, |p, x, y| p.road_tile(x, y));
    }
    // Ore: rich fields on the floodplain, fields out on the plateau.
    for (y, dx, r, s) in [
        (46, -13, 4, 2330),
        (64, 13, 4, 2331),
        (104, -14, 5, 2332),
        (116, 14, 4, 2333),
        (30, 40, 4, 2334),
        (60, -44, 4, 2335),
        (112, -50, 4, 2336),
        (100, 46, 5, 2337),
    ] {
        p.ore(nile_x(p, y) + dx, y, r, s);
    }
    p.home_ore(4, 2340);
    p.finish(1, 2341);
}
