//! Six maps made for the engine's largest size, 256 x 256 cells, each around a kind of country
//! the earlier maps do not have (docs/DECISIONS.md, "Maps at the engine's largest size"): a
//! volcano, a frozen sound, open farmland, a swamp, a lake district, and a wheel of five
//! environments. They use the grounds added with them: ice and marsh (units cross, nothing is
//! built), fields in crop, volcanic ash and lava.
use super::*;

pub fn caldera_biome(p: &Painter, x: i32, y: i32) -> Biome {
    // Ash and basalt on the mountain, jungle on its lower slopes.
    let r = 86 + p.wob(x, y, 20, 700, 8);
    if centre_d2(p, x, y) <= r * r {
        VOLCANIC
    } else {
        TROPIC
    }
}

/// Ember Caldera, 256 x 256: a volcanic island. A lava lake fills the crater at its centre; a
/// ring wall of basalt with four passes encloses the caldera floor and its rich ore. Four lava
/// rivers run from the crater to the sea and cut the island into quarters, crossed only where
/// the flow has cooled: a ford on the caldera floor and a broad one on the ring road. Ash and
/// basalt above, jungle, palms and fishing villages on the lower slopes, the sea all around.
pub fn ember_caldera(p: &mut Painter) {
    let (cx, cy) = (p.w / 2, p.h / 2);
    // The island in its sea.
    p.paint(
        |p, x, y| {
            let r = 116 + p.wob(x, y, 30, 701, 9) + p.wob(x, y, 9, 702, 3);
            centre_d2(p, x, y) > r * r
        },
        WATER,
    );
    // Jungle below, thin woods on the ash, palms on the shore.
    p.fill(
        |p, x, y| {
            let threshold = if p.biome(x, y).plain == ASH { 715 } else { 585 };
            p.is_plain(p.get(x, y)) && p.noise(x, y, 16, 703) > threshold
        },
        |p, x, y| p.biome(x, y).forest,
    );
    p.paint(
        |p, x, y| {
            p.get(x, y) == GRASS
                && p.noise(x, y, 6, 716) > 640
                && (-3..=3).any(|d| p.is_water(x + d, y) || p.is_water(x, y + d))
        },
        PALMS,
    );
    // Ribs of basalt down the ash slopes, broken by gullies.
    p.paint(
        |p, x, y| {
            p.get(x, y) == ASH
                && (540..590).contains(&p.noise(x, y, 18, 704))
                && p.noise(x, y, 30, 705) > 450
        },
        BASALT,
    );
    // The caldera: floor, ring wall with a pass on each side, crater rim and lava lake.
    p.paint(|p, x, y| centre_d2(p, x, y) <= 47 * 47, ASH);
    p.paint(
        |p, x, y| {
            let r = 49 + p.wob(x, y, 12, 706, 2);
            let (u, v) = p.centred(x, y);
            ((r - 3) * (r - 3)..=(r + 3) * (r + 3)).contains(&centre_d2(p, x, y))
                && u.abs() > 24
                && v.abs() > 24
        },
        BASALT,
    );
    p.paint(
        |p, x, y| centre_d2(p, x, y) <= (22 + p.wob(x, y, 8, 707, 2)).pow(2),
        BASALT,
    );
    p.paint(
        |p, x, y| centre_d2(p, x, y) <= (17 + p.wob(x, y, 8, 708, 2)).pow(2),
        LAVA,
    );
    // The lava rivers to the sea (each with its mirror), and where they have cooled.
    let flow = |p: &Painter, x: i32, y: i32| {
        if p.is_water(x, y) { WATER } else { LAVA }
    };
    p.stroke_with(
        &[
            (112, 112),
            (103, 103),
            (84, 90),
            (64, 64),
            (56, 44),
            (36, 40),
            (20, 20),
        ],
        3,
        flow,
    );
    p.stroke_with(
        &[
            (143, 112),
            (152, 103),
            (165, 84),
            (191, 64),
            (196, 44),
            (216, 42),
            (235, 20),
        ],
        3,
        flow,
    );
    for (x, y, r, s) in [
        (103, 103, 5, 709),
        (152, 103, 5, 710),
        (64, 64, 8, 711),
        (191, 64, 8, 712),
    ] {
        p.blob(
            x,
            y,
            r,
            1,
            s,
            |p, x, y| {
                if p.is_water(x, y) { WATER } else { ASH }
            },
        );
    }
    for (x, y) in [(60, 20), (196, 20), (20, 60), (20, 196), (128, 14)] {
        p.village(x, y, 4, 713);
    }
    p.landmarks(18, 14, 714, |p, x, y| centre_d2(p, x, y) > 54 * 54);
    // The ring road around the mountain, and the roads up to the passes.
    p.fill(
        |p, x, y| !p.is_water(x, y) && (89 * 89..=92 * 92).contains(&centre_d2(p, x, y)),
        |p, x, y| p.road_tile(x, y),
    );
    p.road_y(cx - 1, 38, 88, 2);
    p.road_x(cy - 1, 38, 88, 2);
    // Ore: rich fields on the caldera floor, fields on the slopes.
    for (x, y, r, s) in [
        (128, 94, 5, 717),
        (94, 128, 5, 718),
        (128, 60, 4, 719),
        (60, 128, 4, 720),
        (70, 30, 4, 721),
        (30, 70, 4, 722),
        (186, 30, 4, 723),
        (30, 186, 4, 724),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 740);
    p.finish(2, 725);
}

/// Frozen Sound, 256 x 256: two snowbound shores and a sheet of sea ice between them. Armies
/// cross the ice anywhere it holds, but nothing can be built on it, so the middle of the map is
/// open manoeuvre ground cut by leads of dark water and studded with icebergs. Islands frozen
/// into the sheet hold ore and the ruins of a research station; each shore has pine woods,
/// mountains at its back and a harbour town.
pub fn frozen_sound(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    let (cx, cy) = (w / 2, h / 2);
    // The sound: ice from shore to shore, open ocean at its western and eastern ends.
    p.paint(
        |p, x, y| y.min(p.h - 1 - y) >= 66 + p.wob(x, y, 30, 760, 10) + p.wob(x, y, 8, 761, 3),
        ICE,
    );
    p.paint(
        |p, x, y| p.get(x, y) == ICE && x.min(p.w - 1 - x) < 12 + p.wob(x, y, 20, 762, 6),
        WATER,
    );
    // Leads: cracks of open water through the sheet, and icebergs frozen into it.
    p.paint(
        |p, x, y| {
            p.get(x, y) == ICE
                && (498..530).contains(&p.noise(x, y, 26, 763))
                && p.noise(x, y, 40, 764) > 440
        },
        WATER,
    );
    p.paint(
        |p, x, y| p.get(x, y) == ICE && p.noise(x, y, 5, 765) > 800,
        ICEBERG,
    );
    // Islands in the ice.
    for (x, y, r, s) in [(cx, cy, 20, 766), (70, 112, 11, 767), (186, 104, 10, 768)] {
        p.blob(x, y, r, r / 4, s, |_, _, _| SNOW);
    }
    // Mountains at the back of each shore and spurs between the bases; pines.
    p.paint(
        |p, x, y| y.min(p.h - 1 - y) < 7 + p.wob(x, y, 14, 769, 4),
        SNOW_ROCK,
    );
    p.paint(
        |p, x, y| p.get(x, y) == SNOW && p.noise(x, y, 11, 770) > 770,
        SNOW_ROCK,
    );
    p.forests(15, 771, 615, |_, _| true);
    // The harbour towns, and the research station on the middle island.
    p.city(
        |p, x, y| {
            let d = |cx: i32, cy: i32| (x - cx).pow(2) + (y - cy).pow(2) <= 12 * 12;
            let (mx, my) = p.mirror_of(128, 56);
            (d(128, 56) || d(mx, my)) && p.get(x, y) != ICE
        },
        128,
        56,
        5,
        5,
        772,
    );
    p.city(|p, x, y| centre_d2(p, x, y) <= 11 * 11, cx, cy, 4, 5, 773);
    for (x, y) in [(52, 44), (204, 40), (100, 30), (160, 34), (70, 110)] {
        p.village(x, y, 3, 774);
    }
    p.landmarks(18, 14, 775, |_, _, _| true);
    // Three ways over the ice that no lead or iceberg closes.
    let hold = |p: &Painter, x: i32, y: i32| match p.get(x, y) {
        WATER | ICEBERG => ICE,
        tile => tile,
    };
    p.stroke_with(&[(44, 60), (58, 127), (44, 195)], 5, hold);
    p.stroke_with(&[(127, 62), (127, 193)], 5, hold);
    // The shore road and the spurs to the bases.
    p.road_x(47, 16, w - 17, 2);
    for x in [30, 78, 126, 174, 222] {
        p.road_y(x, 34, 47, 2);
    }
    // Ore: on the islands in the ice and along the shores.
    for (x, y, r, s) in [
        (72, 112, 4, 776),
        (186, 104, 4, 777),
        (112, 120, 4, 778),
        (54, 56, 4, 779),
        (102, 52, 4, 780),
        (152, 54, 4, 781),
        (200, 56, 4, 782),
    ] {
        p.ore(x, y, r, s);
    }
    // Gems on the island in the middle of the ice.
    p.gem_seam(112, 120);
    p.home_ore(4, 783);
    p.finish(1, 784);
}

/// Harvest Plains, 256 x 256: farmland to the horizon. A patchwork of grain fields, ploughed
/// earth and pasture, hedgerows with gaps, farm tracks and villages: open tank country with
/// little to hide behind. A river winds from west to east through the middle between marshy
/// banks, crossed by three bridges (one in the market town at the centre) and two broad fords.
pub fn harvest_plains(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    let (cx, cy) = (w / 2, h / 2);
    // The patchwork: fields of 16 x 12 cells, each in crop, ploughed or pasture.
    let field = |p: &Painter, x: i32, y: i32| {
        let (u, v) = p.centred(x, y);
        ((u + 16).div_euclid(32), (v + 12).div_euclid(24))
    };
    p.fill(
        |_, _, _| true,
        |p, x, y| {
            let (bu, bv) = field(p, x, y);
            match hash(bu, bv, 800) % 10 {
                0..=3 => CROPS,
                4 | 5 => DIRT,
                _ => GRASS,
            }
        },
    );
    // Hedgerows along some of the field edges, with a gap every few cells.
    p.paint(
        |p, x, y| {
            let (u, v) = p.centred(x, y);
            let (bu, bv) = field(p, x, y);
            let west = (u + 16).rem_euclid(32) == 1 && hash(bu, bv, 801).is_multiple_of(3);
            let north = (v + 12).rem_euclid(24) == 1 && hash(bu, bv, 802).is_multiple_of(3);
            (west && (y / 2) % 5 != 0) || (north && (x / 2) % 5 != 0)
        },
        ORCHARD,
    );
    // Woods, and two low hills.
    p.forests(20, 803, 700, |_, _| true);
    p.blob(60, 96, 6, 3, 804, |_, _, _| ROCK);
    p.blob(196, 60, 5, 3, 805, |_, _, _| ROCK);
    // The river, which is its own point mirror, and its marshes.
    let off = |p: &Painter, x: i32, y: i32| {
        let (_, v) = p.centred(x, y);
        (v - 2 * p.meander(x, p.w, 40, 806, 22)).abs()
    };
    p.paint(|p, x, y| off(p, x, y) <= 7, WATER);
    p.paint(
        |p, x, y| {
            (8..=20).contains(&off(p, x, y))
                && p.noise(x, y, 10, 807) > 460
                && centre_d2(p, x, y) > 26 * 26
        },
        MARSH,
    );
    // The market town at the centre, and villages among the fields.
    p.city(
        |p, x, y| centre_d2(p, x, y) <= (19 + p.wob(x, y, 8, 808, 2)).pow(2) && !p.is_water(x, y),
        cx,
        cy,
        7,
        5,
        809,
    );
    for (x, y) in [
        (76, 44),
        (180, 40),
        (40, 96),
        (128, 70),
        (216, 100),
        (100, 100),
    ] {
        p.village(x, y, 4, 810);
    }
    p.landmarks(24, 15, 811, |p, x, y| centre_d2(p, x, y) > 28 * 28);
    // Fords: gravel bars where the river runs wide and shallow.
    let bar = |p: &Painter, x: i32, y: i32| match p.get(x, y) {
        WATER | MARSH => SAND,
        tile => tile,
    };
    p.stroke_with(&[(40, 92), (40, 164)], 6, bar);
    p.stroke_with(&[(196, 92), (196, 164)], 6, bar);
    // Roads: the highways north and south of the river, the bridges, and farm tracks.
    p.road_x(57, 16, w - 17, 2);
    p.road_y(cx - 1, 16, h - 17, 2);
    p.road_y(83, 30, h - 31, 3);
    p.road_y(29, 30, 57, 2);
    p.road_y(221, 38, 57, 2);
    p.road_x(83, 60, 196, 2);
    // Ore.
    for (x, y, r, s) in [
        (100, 60, 4, 812),
        (156, 62, 4, 813),
        (60, 110, 4, 814),
        (200, 80, 4, 815),
        (104, 108, 5, 816),
        (20, 70, 4, 817),
        (236, 70, 4, 818),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 819);
    p.finish(1, 820);
}

/// Black Water, 256 x 256: a swamp. Nearly all of it is marsh, which armies wade through and
/// nobody can build on; bayous of open water and mangrove thickets make a maze of it, and
/// timber causeways run straight across. The only building ground is on dry hummocks: one for
/// each base, and smaller ones worth fighting for. A drowned city lies in the middle.
pub fn black_water(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    let (cx, cy) = (w / 2, h / 2);
    p.paint(|_, _, _| true, MARSH);
    // The sea on the western and eastern edges, bayous winding in from it, and pools.
    p.paint(
        |p, x, y| x.min(p.w - 1 - x) < 7 + p.wob(x, y, 18, 830, 4),
        WATER,
    );
    p.paint(
        |p, x, y| (585..640).contains(&p.noise(x, y, 30, 831)) && p.noise(x, y, 22, 832) > 430,
        WATER,
    );
    p.paint(|p, x, y| p.noise(x, y, 12, 833) > 790, WATER);
    // Mangrove thickets.
    p.paint(
        |p, x, y| p.get(x, y) == MARSH && p.noise(x, y, 10, 834) > 650,
        MANGROVE,
    );
    // Dry hummocks: one under every base, and those the noise raises.
    p.paint(
        |p, x, y| p.get(x, y) != WATER && p.noise(x, y, 24, 835) > 730,
        GRASS,
    );
    let homes: Vec<(i32, i32)> = p.starts.iter().copied().step_by(2).collect();
    for (k, &(sx, sy)) in homes.iter().enumerate() {
        p.blob(sx + 1, sy + 1, 19, 5, 836 + k as u32, |_, _, _| GRASS);
    }
    for (x, y, r, s) in [
        (84, 70, 9, 842),
        (170, 66, 9, 843),
        (60, 150, 8, 844),
        (128, 96, 8, 845),
    ] {
        p.blob(x, y, r, 3, s, |_, _, _| GRASS);
    }
    p.forests(9, 846, 700, |_, _| true);
    // The drowned city: its streets are marsh and water, half its blocks have fallen in.
    p.city(|p, x, y| centre_d2(p, x, y) <= 24 * 24, cx, cy, 9, 6, 847);
    p.fill(
        |p, x, y| centre_d2(p, x, y) <= 24 * 24,
        |p, x, y| {
            let (kx, ky) = canonical(p, x, y);
            let roll = hash(kx / 3, ky / 3, 848) % 10;
            match p.get(x, y) {
                STREET if p.noise(x, y, 7, 849) > 560 => WATER,
                STREET | GRASS => MARSH,
                CITY | TOWER if roll < 3 => MARSH,
                CITY | TOWER if roll < 5 => MANGROVE,
                FOREST => MANGROVE,
                tile => tile,
            }
        },
    );
    for (x, y) in [(84, 70), (170, 66), (60, 150), (128, 96)] {
        p.village(x, y, 3, 850);
    }
    p.landmarks(20, 15, 851, |_, _, _| true);
    // Causeways: straight timber roads over marsh and water alike.
    p.road_x(cy - 1, 12, w - 13, 3);
    p.road_y(cx - 1, 12, h - 13, 3);
    p.road_x(47, 28, w - 29, 2);
    p.road_y(47, 28, h - 29, 2);
    p.road_y(207, 28, cy, 2);
    p.road_x(95, 47, cx, 2);
    // Ore: on the hummocks, and lying in the mud between them.
    for (x, y, r, s) in [
        (86, 72, 4, 852),
        (170, 68, 4, 853),
        (62, 152, 4, 854),
        (128, 98, 4, 855),
        (104, 40, 4, 856),
        (40, 104, 4, 857),
        (150, 110, 4, 858),
        (200, 110, 4, 859),
    ] {
        p.ore(x, y, r, s);
    }
    // Gems on a hummock in the middle of the marsh.
    p.gem_seam(150, 110);
    p.home_ore(4, 860);
    p.finish(1, 861);
}

pub fn lakeland_biome(p: &Painter, x: i32, y: i32) -> Biome {
    // Snowy fells in the north-west and south-east corners.
    if (x + y - (p.w - 1)).abs() > 160 + p.wob(x, y, 22, 870, 10) {
        ARCTIC
    } else {
        TEMPERATE
    }
}

/// Thousand Lakes, 256 x 256: a lake district. Lakes of every size lie everywhere, frozen over
/// on the snowy fells in two corners, and the land between them is a web of necks and
/// peninsulas with many ways round and none of them straight. Moraine ridges, forests and
/// farmland fill it; roads cross the lakes on long bridges, and a lakeside town stands at the
/// centre.
pub fn thousand_lakes(p: &mut Painter) {
    let (w, h) = (p.w, p.h);
    let (cx, cy) = (w / 2, h / 2);
    // Lakes great and small; on the fells they are frozen.
    p.paint(|p, x, y| p.noise(x, y, 30, 871) > 655, WATER);
    p.paint(|p, x, y| p.noise(x, y, 13, 872) > 735, WATER);
    p.paint(
        |p, x, y| p.is_water(x, y) && p.biome(x, y).plain == SNOW,
        ICE,
    );
    // Marsh at the heads of the lakes.
    p.paint(
        |p, x, y| {
            p.get(x, y) == GRASS
                && (625..=655).contains(&p.noise(x, y, 30, 871))
                && p.noise(x, y, 9, 873) > 540
        },
        MARSH,
    );
    // Moraine ridges, cut by gaps.
    p.fill(
        |p, x, y| {
            p.is_plain(p.get(x, y))
                && (430..462).contains(&p.noise(x, y, 24, 874))
                && p.noise(x, y, 16, 875) > 500
        },
        |p, x, y| p.biome(x, y).cliff,
    );
    // Dry ground under every base, whatever lake lay there.
    let homes: Vec<(i32, i32)> = p.starts.iter().copied().step_by(2).collect();
    for (k, &(sx, sy)) in homes.iter().enumerate() {
        p.blob(sx + 1, sy + 1, 14, 4, 876 + k as u32, |p, x, y| {
            p.biome(x, y).plain
        });
    }
    farmland(p, 14, 882, 650);
    p.forests(17, 883, 600, |_, _| true);
    // The lakeside town, and villages.
    p.city(
        |p, x, y| centre_d2(p, x, y) <= (17 + p.wob(x, y, 8, 884, 3)).pow(2) && !p.is_water(x, y),
        cx,
        cy,
        6,
        5,
        885,
    );
    for (x, y) in [
        (70, 60),
        (180, 50),
        (40, 110),
        (120, 30),
        (220, 90),
        (96, 96),
    ] {
        p.village(x, y, 4, 886);
    }
    p.landmarks(20, 15, 887, |p, x, y| centre_d2(p, x, y) > 24 * 24);
    // Roads: straight over land and lake.
    p.road_x(cy - 1, 12, w - 13, 2);
    p.road_y(cx - 1, 12, h - 13, 2);
    p.road_x(45, 24, w - 25, 2);
    p.road_y(45, 24, h - 25, 2);
    p.road_x(87, 45, w - 46, 2);
    p.road_y(87, 45, h - 46, 2);
    // Ore.
    for (x, y, r, s) in [
        (100, 110, 5, 888),
        (70, 70, 4, 889),
        (180, 66, 4, 890),
        (110, 50, 4, 891),
        (50, 110, 4, 892),
        (150, 96, 4, 893),
        (214, 110, 4, 894),
    ] {
        p.ore(x, y, r, s);
    }
    p.home_ore(4, 895);
    p.finish(2, 896);
}

/// Directions of the wheel's ten spokes (the borders between its sectors) and of the middle of
/// each sector, as (cos, sin) x 1000 of 36 k and 18 + 36 k degrees.
const SPOKES: [(i32, i32); 10] = [
    (1000, 0),
    (809, 588),
    (309, 951),
    (-309, 951),
    (-809, 588),
    (-1000, 0),
    (-809, -588),
    (-309, -951),
    (309, -951),
    (809, -588),
];
const MIDDLES: [(i32, i32); 10] = [
    (951, 309),
    (588, 809),
    (0, 1000),
    (-588, 809),
    (-951, 309),
    (-951, -309),
    (-588, -809),
    (0, -1000),
    (588, -809),
    (951, -309),
];

/// The sector of the wheel a cell lies in, 0 to 9. The borders wander, and a cell's mirror lies
/// in the opposite sector, which has the same environment.
fn sector(p: &Painter, x: i32, y: i32) -> usize {
    // A displacement that is odd about the centre, so that the wandering borders mirror.
    let odd = |seed: u32| {
        let d = fbm(x, y, 26, seed) - fbm(p.w - 1 - x, p.h - 1 - y, 26, seed);
        (d * 9 / 256) as i32
    };
    let (u, v) = p.centred(x, y);
    let (u, v) = (i64::from(u + odd(900)), i64::from(v + odd(901)));
    let side = |k: usize| {
        let (c, s) = SPOKES[k % 10];
        i64::from(c) * v - i64::from(s) * u
    };
    (0..10)
        .find(|&k| side(k) >= 0 && side(k + 1) < 0)
        .unwrap_or(0)
}

pub fn wheel_biome(p: &Painter, x: i32, y: i32) -> Biome {
    [TEMPERATE, ARCTIC, VOLCANIC, DESERT_BIOME, TROPIC][sector(p, x, y) % 5]
}

/// The cell `r` cells from the centre in the middle of sector `k`, turned by `turn` cells
/// along the circle.
fn spoke(p: &Painter, k: usize, r: i32, turn: i32) -> (i32, i32) {
    let (c, s) = MIDDLES[k % 10];
    (
        p.w / 2 + (r * c - turn * s) / 1000,
        p.h / 2 + (r * s + turn * c) / 1000,
    )
}

/// Wheel of Worlds, 256 x 256: ten sectors around a ruined city, two of each of five
/// environments (farmland, snow, volcanic ash, desert, jungle), opposite sectors alike. A ridge
/// of the local rock runs down every border, open only at the hub and where the ring road
/// passes through; every sector holds one base and its own kind of country.
pub fn wheel_of_worlds(p: &mut Painter) {
    let (cx, cy) = (p.w / 2, p.h / 2);
    // The ridges on the borders between the sectors.
    p.fill(
        |p, x, y| {
            let d2 = centre_d2(p, x, y);
            let here = sector(p, x, y);
            let border = [(2, 0), (-2, 0), (0, 2), (0, -2), (1, 1), (-1, -1)]
                .iter()
                .any(|&(dx, dy)| sector(p, x + dx, y + dy) != here);
            let open = d2 < 46 * 46 || (71 * 71..=91 * 91).contains(&d2);
            border && !open
        },
        |p, x, y| p.biome(x, y).cliff,
    );
    // What each kind of sector is made of (the mirror paints the opposite one).
    for k in 5..10 {
        let at = |r: i32, turn: i32| spoke(p, k, r, turn);
        let (near, far, side) = (at(58, 6), at(116, -14), at(100, 26));
        match k % 5 {
            // Farmland: fields, a mill pond, an orchard.
            0 => {
                p.patch(near.0, near.1, 9, 3, 910, CROPS);
                p.patch(far.0, far.1, 8, 3, 911, CROPS);
                p.patch(side.0, side.1, 4, 2, 912, WATER);
                p.village(near.0 + 3, near.1 - 2, 4, 913);
            }
            // Snow: a frozen lake, a tarn, crags.
            1 => {
                p.patch(near.0, near.1, 10, 3, 914, ICE);
                p.patch(side.0, side.1, 5, 2, 915, WATER);
                p.patch(far.0, far.1, 4, 2, 916, SNOW_ROCK);
            }
            // Ash: a lava flow from a vent, basalt.
            2 => {
                p.stroke(&[at(50, 0), at(64, 8), at(78, 4)], 2, LAVA);
                p.patch(far.0, far.1, 5, 2, 917, BASALT);
                p.blob(side.0, side.1, 3, 1, 918, |_, _, _| LAVA);
            }
            // Desert: mesas, an oasis.
            3 => {
                p.patch(near.0, near.1, 6, 3, 919, MESA);
                p.patch(far.0, far.1, 7, 2, 920, DIRT);
                p.blob(far.0, far.1, 4, 1, 921, |_, _, _| PALMS);
                p.blob(far.0, far.1, 2, 1, 922, |_, _, _| WATER);
            }
            // Jungle: a lagoon in its mangroves.
            _ => {
                p.patch(near.0, near.1, 11, 3, 923, MARSH);
                p.blob(near.0, near.1, 6, 2, 924, |_, _, _| WATER);
                p.patch(side.0, side.1, 6, 2, 925, MARSH);
            }
        }
    }
    p.paint(
        |p, x, y| p.get(x, y) == MARSH && p.noise(x, y, 4, 926) > 640,
        MANGROVE,
    );
    // Woods of every kind: thick in the jungle, thin on the ash and the sand.
    p.fill(
        |p, x, y| {
            let threshold = match p.biome(x, y).plain {
                ASH | DESERT => 720,
                SNOW => 630,
                _ if sector(p, x, y) % 5 == 4 => 540,
                _ => 640,
            };
            p.is_plain(p.get(x, y))
                && centre_d2(p, x, y) > 44 * 44
                && p.noise(x, y, 15, 927) > threshold
        },
        |p, x, y| p.biome(x, y).forest,
    );
    // The ruined city at the hub: every sector's quarter in its own stone.
    p.city(
        |p, x, y| centre_d2(p, x, y) <= (21 + p.wob(x, y, 8, 928, 2)).pow(2),
        cx,
        cy,
        8,
        6,
        929,
    );
    p.fill(
        |p, x, y| centre_d2(p, x, y) <= 24 * 24 && !is_urban(p.get(x, y)),
        |p, x, y| p.biome(x, y).plain,
    );
    p.fill(
        |p, x, y| {
            let (kx, ky) = canonical(p, x, y);
            centre_d2(p, x, y) <= 24 * 24
                && is_urban(p.get(x, y))
                && ![STREET, PLAZA].contains(&p.get(x, y))
                && hash(kx / 2, ky / 2, 930) % 10 < 3
        },
        |_, _, _| PLAZA,
    );
    for k in 5..10 {
        let (x, y) = spoke(p, k, 112, 20);
        p.village(x, y, 3, 931);
    }
    p.landmarks(20, 15, 932, |p, x, y| centre_d2(p, x, y) > 48 * 48);
    // The ring road through the gaps in the ridges, and a road from the hub out to it.
    p.fill(
        |p, x, y| (80 * 80..=83 * 83).contains(&centre_d2(p, x, y)),
        |p, x, y| p.road_tile(x, y),
    );
    p.road_y(cx - 1, 46, 82, 2);
    // Ore: a ring of fields around the hub, one out in every sector.
    for k in 5..10 {
        for (r, turn, size) in [(33, 0, 4), (104, -24, 4), (62, -16, 3)] {
            let (x, y) = spoke(p, k, r, turn);
            p.ore(x, y, size, 933 + k as u32);
            // Gems in the ring of fields around the hub.
            if r == 33 {
                p.gem_seam(x, y);
            }
        }
    }
    p.home_ore(4, 950);
    p.finish(1, 951);
}
