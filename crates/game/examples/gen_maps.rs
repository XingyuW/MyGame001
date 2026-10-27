//! Generates the skirmish maps `assets/maps/<id>/<id>.tmx` with their tilesets
//! `assets/maps/<id>/<id>_tiles.png` (docs/DECISIONS.md D50 and "Large maps with environments").
//!
//! Run with `cargo run -p game --example gen_maps`. The output is deterministic (integer
//! arithmetic only), and `crates/game/tests/m7_maps.rs` checks that the committed files match it
//! byte for byte.
//!
//! Every map is point-symmetric about its centre, so that mirrored start positions are equal: the
//! cell (x, y) has the terrain of (W − 1 − x, H − 1 − y). Each start has buildable ground for a
//! base and ore fields nearby; contested ore lies between the bases.
//!
//! * `crossroads` (56 × 56): starts in opposite corners, rock ridges and two lakes that leave
//!   three lanes across the map.
//! * `riverlands` (64 × 48): starts on the west and east banks of a river that can be crossed at
//!   three fords.
//!
//! The large maps (`art/world_maps.rs`) are painted at or near the largest size a map may have
//! (`sim::MAX_MAP_SIDE`, 256 cells a side), so that bases lie far apart, and mix environments,
//! each with cities, coasts and nature (docs/DECISIONS.md, "Maps at the engine's largest size"):
//!
//! * `liberty_bay` (256 × 256): a metropolis on a river between two bays, a port town on each.
//! * `frostline` (224 × 224): arctic snowfields and a mountain chain with an industrial town.
//! * `oasis_gulf` (256 × 192): desert canyons and oases between two gulf coasts, a harbour city.
//! * `coral_archipelago` (256 × 256): tropical islands joined by broad causeways.
//! * `continental_divide` (256 × 256): snow, desert and farmland around a lake with an island city.
//! * `steel_harbor` (256 × 256): a whole-map city on a river between two harbour basins.
//! * `glacier_pass` (240 × 240): green valleys either side of a snowy massif, crossed by passes.
//! * `red_canyon` (192 × 192): a mesa cut by canyons around a ruined desert town, with oases.
//! * `jungle_delta` (256 × 224): a tropical delta around a lagoon, jungle regions joined by sand bars.
//!
//! The ocean maps keep the land a tree (no loop of land encloses water), so that the sea is one
//! piece and fleets can reach every shore, while every base still has a way to the others by land:
//!
//! * `pacific_atoll` (256 × 256): a reef ring around a lagoon in the open ocean, cut by two channels.
//! * `iron_isthmus` (256 × 224): two continents in the sea, joined by one neck of land with a port town.
//! * `arctic_floes` (240 × 240): snow islands in a polar sea, joined by shelves of sea ice.
//!
//! Six maps (`art/frontier_maps.rs`) were made for the largest size and the grounds added with
//! them (ice, marsh, fields in crop, volcanic ash, lava):
//!
//! * `ember_caldera`: a volcanic island quartered by lava rivers around a crater lake of lava.
//! * `frozen_sound`: two snowbound shores and a sheet of sea ice between them.
//! * `harvest_plains`: a patchwork of fields, hedgerows and villages, and a river across it.
//! * `black_water`: a swamp with building ground only on dry hummocks, and a drowned city.
//! * `thousand_lakes`: a lake district, a web of necks and peninsulas between the lakes.
//! * `wheel_of_worlds`: ten sectors of five environments around a ruined city.
//!
//! Twenty-one more (2026-10-26), each around a landscape the others lack, keep their size and
//! homes beside their recipes (`world_maps::Layout`) in five files by kind of country:
//!
//! * `art/arid_maps.rs`: `dune_sea`, `badlands`, `oil_fields` (256 × 192), `nile_valley`.
//! * `art/water_maps.rs`: `canal_city`, `polder_lowlands` (256 × 224), `inland_sea`,
//!   `mangrove_coast` (256 × 224), `coastal_cliffs` (240 × 240).
//! * `art/cold_maps.rs`: `fjord_coast`, `tundra_steppe`, `ice_shelf_rift` (240 × 240).
//! * `art/green_maps.rs`: `savanna`, `river_confluence`, `old_growth`, `rice_terraces`
//!   (224 × 224), `suburbia`.
//! * `art/rugged_maps.rs`: `river_gorge`, `twin_volcanoes`, `crater_field` (240 × 240),
//!   `highland_plateau`.
//!
//! The rest are 256 × 256, except `badlands` (224 × 224).
//!
//! `gen_maps check [id ...]` paints the named maps (all if none) and checks the rules of
//! `tests/m7_maps.rs` that need no files: symmetry, base sites, ore near every start, starts
//! apart, and every passable cell reachable by land. It is for working on a recipe; the test is
//! the authority.
//!
//! `gen_maps preview <dir> [id ...]` writes a picture of each map (three pixels a cell) for
//! reviewing a recipe, and `gen_maps starts` prints the map list of `assets/data/skirmish.ron`.
//!
//! Every tile carries a presentation-only `look` (`art/looks.rs`), which tells the renderer the
//! ground and what stands on it: the same impassable terrain is a forest, a city block or a cliff.
//! The tileset is textured: every tile kind carries a fixed pattern of lighter and darker pixels
//! from an integer hash, and an outline, so that cells stay distinguishable.

#[path = "art/battlefield_layout.rs"]
mod battlefield_layout;
#[path = "art/looks.rs"]
#[allow(dead_code)]
pub mod looks;
#[path = "art/world_maps.rs"]
pub mod world_maps;
use looks::{Feature as F, Ground as G, look};
use std::fmt::Write as _;
use std::path::PathBuf;

#[path = "gen_spike_tileset.rs"]
#[allow(dead_code)]
mod spike_tileset;

pub const TILE_WIDTH: u32 = 64;
pub const TILE_HEIGHT: u32 = 32;

/// A tile of the tileset: name, base colour, the terrain properties of the map format
/// (docs/DECISIONS.md, D8), and its look.
pub struct TileKind {
    pub name: &'static str,
    pub colour: [u8; 3],
    pub passable: bool,
    pub buildable: bool,
    pub ore: i32,
    pub water: bool,
    pub look: u8,
}

const fn kind(
    name: &'static str,
    colour: [u8; 3],
    (passable, buildable, ore): (bool, bool, i32),
    ground: G,
    feature: F,
) -> TileKind {
    TileKind {
        name,
        colour,
        passable,
        buildable,
        ore,
        water: matches!(ground, G::Water) && !matches!(feature, F::Bridge),
        look: look(ground, feature),
    }
}

/// Open ground, ore ground, roads, and blocked ground (properties: passable, buildable, ore).
const OPEN: (bool, bool, i32) = (true, true, 0);
const ORE_FIELD: (bool, bool, i32) = (true, false, 500);
/// Gems (RA5): ore ground whose look says gems. An amount of 600 is twelve bails of 50 credits,
/// the densest a cell is (`sim`'s match setup turns the amounts into bails).
const GEM_FIELD: (bool, bool, i32) = (true, false, 600);
const WAY: (bool, bool, i32) = (true, false, 0);
const BLOCKED: (bool, bool, i32) = (false, false, 0);

/// The tileset, indexed by tile id.
pub const TILES: [TileKind; 57] = [
    kind("grass", [74, 132, 58], OPEN, G::Grass, F::None),
    kind("ore", [196, 158, 44], ORE_FIELD, G::Grass, F::Ore),
    kind("water", [38, 86, 160], BLOCKED, G::Water, F::None),
    kind("rock", [112, 104, 96], BLOCKED, G::Grass, F::Cliff),
    kind("sand", [188, 172, 118], WAY, G::Sand, F::None),
    kind("road", [86, 86, 84], WAY, G::Grass, F::Road),
    kind("forest", [40, 84, 40], BLOCKED, G::Grass, F::Forest),
    kind("cottage", [150, 104, 84], BLOCKED, G::Grass, F::House),
    kind("dirt", [122, 100, 70], OPEN, G::Dirt, F::None),
    kind("desert", [200, 178, 124], OPEN, G::Sand, F::None),
    kind("snow", [224, 228, 236], OPEN, G::Snow, F::None),
    kind("plaza", [150, 150, 150], OPEN, G::Pavement, F::None),
    kind("desert ore", [210, 168, 60], ORE_FIELD, G::Sand, F::Ore),
    kind("snow ore", [214, 186, 92], ORE_FIELD, G::Snow, F::Ore),
    kind("mesa", [168, 104, 66], BLOCKED, G::Sand, F::Cliff),
    kind("snowy rock", [150, 158, 170], BLOCKED, G::Snow, F::Cliff),
    kind("palms", [96, 128, 58], BLOCKED, G::Sand, F::Forest),
    kind("snowy pines", [64, 92, 84], BLOCKED, G::Snow, F::Forest),
    kind("city block", [118, 118, 126], BLOCKED, G::Pavement, F::City),
    kind("tower", [170, 172, 184], BLOCKED, G::Pavement, F::Tower),
    kind("bridge", [128, 112, 92], WAY, G::Water, F::Bridge),
    kind("street", [84, 84, 86], WAY, G::Pavement, F::Road),
    kind("snowy road", [120, 124, 130], WAY, G::Snow, F::Road),
    kind("desert road", [140, 124, 96], WAY, G::Sand, F::Road),
    kind("snowy cottage", [170, 160, 150], BLOCKED, G::Snow, F::House),
    kind("adobe house", [196, 160, 116], BLOCKED, G::Sand, F::House),
    kind("ice", [176, 204, 220], WAY, G::Ice, F::None),
    kind("crops", [196, 172, 84], OPEN, G::Crops, F::None),
    kind("marsh", [70, 88, 56], WAY, G::Marsh, F::None),
    kind("ash", [78, 74, 72], OPEN, G::Ash, F::None),
    kind("lava", [226, 96, 28], BLOCKED, G::Ash, F::Lava),
    kind("ash ore", [176, 140, 50], ORE_FIELD, G::Ash, F::Ore),
    kind("basalt", [52, 48, 50], BLOCKED, G::Ash, F::Cliff),
    kind("ash road", [96, 94, 92], WAY, G::Ash, F::Road),
    kind("ash woods", [50, 76, 44], BLOCKED, G::Ash, F::Forest),
    kind("ash house", [128, 96, 80], BLOCKED, G::Ash, F::House),
    kind("orchard", [70, 104, 48], BLOCKED, G::Dirt, F::Forest),
    kind("farmhouse", [164, 120, 84], BLOCKED, G::Crops, F::House),
    kind("track", [112, 98, 78], WAY, G::Dirt, F::Road),
    kind(
        "snowy city block",
        [150, 154, 164],
        BLOCKED,
        G::Snow,
        F::City,
    ),
    kind("snowy tower", [186, 190, 202], BLOCKED, G::Snow, F::Tower),
    kind(
        "desert city block",
        [176, 150, 116],
        BLOCKED,
        G::Sand,
        F::City,
    ),
    kind("desert tower", [200, 184, 160], BLOCKED, G::Sand, F::Tower),
    kind("mangrove", [44, 78, 52], BLOCKED, G::Marsh, F::Forest),
    kind("iceberg", [196, 214, 228], BLOCKED, G::Ice, F::Cliff),
    kind("marsh ore", [150, 138, 52], ORE_FIELD, G::Marsh, F::Ore),
    kind("gems", [64, 190, 180], GEM_FIELD, G::Grass, F::Gems),
    kind("desert gems", [84, 196, 176], GEM_FIELD, G::Sand, F::Gems),
    kind("snow gems", [112, 206, 206], GEM_FIELD, G::Snow, F::Gems),
    kind("ash gems", [58, 160, 156], GEM_FIELD, G::Ash, F::Gems),
    kind("marsh gems", [56, 150, 132], GEM_FIELD, G::Marsh, F::Gems),
    kind(
        "oil derrick",
        [176, 156, 100],
        BLOCKED,
        G::Pavement,
        F::OilDerrick,
    ),
    kind(
        "hospital",
        [220, 210, 200],
        BLOCKED,
        G::Pavement,
        F::Hospital,
    ),
    kind("airport", [150, 164, 180], BLOCKED, G::Pavement, F::Airport),
    kind(
        "tech outpost",
        [150, 160, 130],
        BLOCKED,
        G::Pavement,
        F::TechOutpost,
    ),
    kind(
        "bridge repair hut",
        [150, 108, 80],
        BLOCKED,
        G::Dirt,
        F::BridgeHut,
    ),
    kind(
        "pier repair hut",
        [150, 108, 80],
        BLOCKED,
        G::Water,
        F::BridgeHut,
    ),
];

pub const GRASS: u32 = 0;
pub const ORE: u32 = 1;
pub const WATER: u32 = 2;
pub const ROCK: u32 = 3;
pub const SAND: u32 = 4;
pub const ROAD: u32 = 5;
pub const FOREST: u32 = 6;
pub const COTTAGE: u32 = 7;
pub const DIRT: u32 = 8;
pub const DESERT: u32 = 9;
pub const SNOW: u32 = 10;
pub const PLAZA: u32 = 11;
pub const DESERT_ORE: u32 = 12;
pub const SNOW_ORE: u32 = 13;
pub const MESA: u32 = 14;
pub const SNOW_ROCK: u32 = 15;
pub const PALMS: u32 = 16;
pub const SNOW_PINES: u32 = 17;
pub const CITY: u32 = 18;
pub const TOWER: u32 = 19;
pub const BRIDGE: u32 = 20;
pub const STREET: u32 = 21;
pub const SNOW_ROAD: u32 = 22;
pub const DESERT_ROAD: u32 = 23;
pub const SNOW_COTTAGE: u32 = 24;
pub const ADOBE: u32 = 25;
pub const ICE: u32 = 26;
pub const CROPS: u32 = 27;
pub const MARSH: u32 = 28;
pub const ASH: u32 = 29;
pub const LAVA: u32 = 30;
pub const ASH_ORE: u32 = 31;
pub const BASALT: u32 = 32;
pub const ASH_ROAD: u32 = 33;
pub const ASH_WOODS: u32 = 34;
pub const ASH_HOUSE: u32 = 35;
pub const ORCHARD: u32 = 36;
pub const FARMHOUSE: u32 = 37;
pub const TRACK: u32 = 38;
pub const SNOW_CITY: u32 = 39;
pub const SNOW_TOWER: u32 = 40;
pub const DESERT_CITY: u32 = 41;
pub const DESERT_TOWER: u32 = 42;
pub const MANGROVE: u32 = 43;
pub const ICEBERG: u32 = 44;
pub const MARSH_ORE: u32 = 45;
pub const GEMS: u32 = 46;
pub const DESERT_GEMS: u32 = 47;
pub const SNOW_GEMS: u32 = 48;
pub const ASH_GEMS: u32 = 49;
pub const MARSH_GEMS: u32 = 50;

/// How a map's terrain is made: a rule per cell (the M7 maps), or a painted recipe
/// (`world_maps::RECIPES`, looked up by the map's id).
pub enum Terrain {
    Rule(fn(i32, i32) -> u32),
    Painted,
}

/// A generated map: its id, size, and homes. Every home is a start position, and so is its point
/// mirror; the map's start positions are the homes with their mirrors, in pairs (0 and 1, 2 and
/// 3, ...; [`MapSpec::starts`], as listed in `assets/data/skirmish.ron`), so that on the
/// symmetric terrain every pair is equal. The first pair is the one-against-one layout: on the
/// large maps the two bases farthest apart. Later pairs (clear building ground, ore within
/// reach, a land path to every other base) let a match have more players.
pub struct MapSpec {
    pub id: &'static str,
    pub width: u32,
    pub height: u32,
    pub homes: &'static [(u32, u32)],
    terrain: Terrain,
}

pub const MAPS: [MapSpec; 41] = [
    MapSpec {
        id: "crossroads",
        width: 56,
        height: 56,
        homes: &[(9, 9), (3, 34), (18, 31)],
        terrain: Terrain::Rule(crossroads),
    },
    MapSpec {
        id: "riverlands",
        width: 64,
        height: 48,
        homes: &[(8, 21), (5, 3)],
        terrain: Terrain::Rule(riverlands),
    },
    MapSpec {
        id: "liberty_bay",
        width: 256,
        height: 256,
        homes: &[(29, 29), (25, 113), (113, 25), (159, 59), (59, 159)],
        terrain: Terrain::Painted,
    },
    MapSpec {
        id: "frostline",
        width: 224,
        height: 224,
        homes: &[(49, 41), (41, 117), (121, 29), (161, 25), (31, 165)],
        terrain: Terrain::Painted,
    },
    MapSpec {
        id: "oasis_gulf",
        width: 256,
        height: 192,
        homes: &[(29, 89), (69, 33), (121, 33), (181, 33), (225, 45)],
        terrain: Terrain::Painted,
    },
    MapSpec {
        id: "coral_archipelago",
        width: 256,
        height: 256,
        homes: &[(41, 41), (129, 25), (25, 113), (193, 57), (229, 17)],
        terrain: Terrain::Painted,
    },
    MapSpec {
        id: "continental_divide",
        width: 256,
        height: 256,
        homes: &[(33, 33), (201, 33), (73, 81), (125, 25), (25, 125)],
        terrain: Terrain::Painted,
    },
    MapSpec {
        id: "steel_harbor",
        width: 256,
        height: 256,
        homes: &[(29, 29), (25, 217), (125, 25), (181, 25), (69, 189)],
        terrain: Terrain::Painted,
    },
    MapSpec {
        id: "glacier_pass",
        width: 240,
        height: 240,
        homes: &[(25, 29), (21, 101), (25, 197), (49, 153)],
        terrain: Terrain::Painted,
    },
    MapSpec {
        id: "red_canyon",
        width: 192,
        height: 192,
        homes: &[(17, 17), (13, 101), (101, 13), (147, 41)],
        terrain: Terrain::Painted,
    },
    MapSpec {
        id: "jungle_delta",
        width: 256,
        height: 224,
        homes: &[(25, 33), (201, 25), (129, 21), (29, 153)],
        terrain: Terrain::Painted,
    },
    MapSpec {
        id: "pacific_atoll",
        width: 256,
        height: 256,
        homes: &[(71, 71), (45, 141), (141, 45)],
        terrain: Terrain::Painted,
    },
    MapSpec {
        id: "iron_isthmus",
        width: 256,
        height: 224,
        homes: &[(45, 45), (109, 37), (33, 109)],
        terrain: Terrain::Painted,
    },
    MapSpec {
        id: "arctic_floes",
        width: 240,
        height: 240,
        homes: &[(45, 45), (137, 37), (37, 137)],
        terrain: Terrain::Painted,
    },
    MapSpec {
        id: "ember_caldera",
        width: 256,
        height: 256,
        homes: &[(95, 33), (33, 95), (159, 33), (33, 159)],
        terrain: Terrain::Painted,
    },
    MapSpec {
        id: "frozen_sound",
        width: 256,
        height: 256,
        homes: &[(29, 31), (221, 31), (125, 27), (77, 25), (173, 25)],
        terrain: Terrain::Painted,
    },
    MapSpec {
        id: "harvest_plains",
        width: 256,
        height: 256,
        homes: &[(29, 29), (221, 37), (125, 21), (69, 81), (181, 77)],
        terrain: Terrain::Painted,
    },
    MapSpec {
        id: "black_water",
        width: 256,
        height: 256,
        homes: &[(33, 33), (215, 45), (125, 25), (37, 125), (120, 70)],
        terrain: Terrain::Painted,
    },
    MapSpec {
        id: "thousand_lakes",
        width: 256,
        height: 256,
        homes: &[(31, 31), (215, 41), (125, 23), (35, 121), (170, 90)],
        terrain: Terrain::Painted,
    },
    MapSpec {
        id: "wheel_of_worlds",
        width: 256,
        height: 256,
        homes: &[(32, 96), (186, 46), (68, 46), (222, 96), (127, 27)],
        terrain: Terrain::Painted,
    },
    MapSpec::painted("dune_sea", world_maps::arid_maps::DUNE_SEA),
    MapSpec::painted("fjord_coast", world_maps::cold_maps::FJORD_COAST),
    MapSpec::painted("canal_city", world_maps::water_maps::CANAL_CITY),
    MapSpec::painted("savanna", world_maps::green_maps::SAVANNA),
    MapSpec::painted("river_gorge", world_maps::rugged_maps::RIVER_GORGE),
    MapSpec::painted("badlands", world_maps::arid_maps::BADLANDS),
    MapSpec::painted("tundra_steppe", world_maps::cold_maps::TUNDRA_STEPPE),
    MapSpec::painted("polder_lowlands", world_maps::water_maps::POLDER_LOWLANDS),
    MapSpec::painted("river_confluence", world_maps::green_maps::RIVER_CONFLUENCE),
    MapSpec::painted("twin_volcanoes", world_maps::rugged_maps::TWIN_VOLCANOES),
    MapSpec::painted("oil_fields", world_maps::arid_maps::OIL_FIELDS),
    MapSpec::painted("ice_shelf_rift", world_maps::cold_maps::ICE_SHELF_RIFT),
    MapSpec::painted("inland_sea", world_maps::water_maps::INLAND_SEA),
    MapSpec::painted("old_growth", world_maps::green_maps::OLD_GROWTH),
    MapSpec::painted("crater_field", world_maps::rugged_maps::CRATER_FIELD),
    MapSpec::painted("nile_valley", world_maps::arid_maps::NILE_VALLEY),
    MapSpec::painted("mangrove_coast", world_maps::water_maps::MANGROVE_COAST),
    MapSpec::painted("rice_terraces", world_maps::green_maps::RICE_TERRACES),
    MapSpec::painted(
        "highland_plateau",
        world_maps::rugged_maps::HIGHLAND_PLATEAU,
    ),
    MapSpec::painted("coastal_cliffs", world_maps::water_maps::COASTAL_CLIFFS),
    MapSpec::painted("suburbia", world_maps::green_maps::SUBURBIA),
];

/// Discrete neutral anchors are point-symmetric and replace existing scenery where possible.
/// They stay out of all start clearings; the three small rule maps keep their historic cells.
fn neutral_sites(spec: &MapSpec, tiles: &mut [u32]) {
    let (w, h) = (spec.width as i32, spec.height as i32);
    let index = |x: i32, y: i32| (y * w + x) as usize;
    let total = tiles.len();
    let mirror = |i: usize| total - 1 - i;
    // Do not block a peninsula neck or strand a small passable pocket with a hut.
    let stays_connected = |x: i32, y: i32, tiles: &[u32]| {
        let neighbors: Vec<_> = [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]
            .into_iter()
            .filter(|&(a, b)| TILES[tiles[index(a, b)] as usize].passable)
            .collect();
        let Some(&first) = neighbors.first() else {
            return true;
        };
        let mut seen = std::collections::BTreeSet::from([first]);
        let mut pending = vec![first];
        while let Some((a, b)) = pending.pop() {
            for next in [(a - 1, b), (a + 1, b), (a, b - 1), (a, b + 1)] {
                let (nx, ny) = next;
                if nx >= 0
                    && ny >= 0
                    && nx < w
                    && ny < h
                    && next != (x, y)
                    && nx.abs_diff(x).max(ny.abs_diff(y)) <= 2
                    && TILES[tiles[index(nx, ny)] as usize].passable
                    && seen.insert(next)
                {
                    pending.push(next);
                }
            }
        }
        neighbors.iter().all(|p| seen.contains(p))
    };
    let mut placed: Vec<(i32, i32)> = Vec::new();
    for n in 0..4 {
        let tx = w * (2 + n % 2) / 5;
        let ty = h * (1 + n / 2) / 5;
        let choice = (2..h - 2)
            .flat_map(|y| (2..w - 2).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let i = index(x, y);
                let j = total - 1 - i;
                let t = &TILES[tiles[i] as usize];
                let m = &TILES[tiles[j] as usize];
                matches!(t.look >> 4, 4..=7)
                    && matches!(m.look >> 4, 4..=7)
                    && spec
                        .starts()
                        .iter()
                        .all(|&(sx, sy)| x.abs_diff(sx as i32).max(y.abs_diff(sy as i32)) > 12)
                    && placed
                        .iter()
                        .all(|&(px, py)| x.abs_diff(px).max(y.abs_diff(py)) > 5)
                    && [(1, 0), (-1, 0), (0, 1), (0, -1)]
                        .into_iter()
                        .filter(|&(dx, dy)| TILES[tiles[index(x + dx, y + dy)] as usize].passable)
                        .count()
                        >= 3
            })
            .min_by_key(|&(x, y)| ((x - tx).pow(2) + (y - ty).pow(2), y, x));
        let Some((x, y)) = choice else {
            panic!("{} has no neutral site {}", spec.id, n)
        };
        let i = index(x, y);
        let j = mirror(i);
        tiles[i] = 51 + n as u32;
        tiles[j] = 51 + n as u32;
        placed.extend([(x, y), (w - 1 - x, h - 1 - y)]);
    }
    // A repair hut at each end of each connected deck run, off the traffic lane.
    let mut remaining: std::collections::BTreeSet<(i32, i32)> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| TILES[tiles[index(x, y)] as usize].look >> 4 == 8)
        .collect();
    while let Some(&first) = remaining.first() {
        let mut pending = vec![first];
        let mut cells = Vec::new();
        remaining.remove(&first);
        while let Some((x, y)) = pending.pop() {
            cells.push((x, y));
            for p in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
                if remaining.remove(&p) {
                    pending.push(p);
                }
            }
        }
        let minx = cells.iter().map(|p| p.0).min().unwrap_or(0);
        let maxx = cells.iter().map(|p| p.0).max().unwrap_or(0);
        let miny = cells.iter().map(|p| p.1).min().unwrap_or(0);
        let maxy = cells.iter().map(|p| p.1).max().unwrap_or(0);
        let horizontal = maxx - minx >= maxy - miny;
        for end in [false, true] {
            let edge = if horizontal {
                if end { maxx } else { minx }
            } else if end {
                maxy
            } else {
                miny
            };
            let across: Vec<_> = cells
                .iter()
                .filter(|p| if horizontal { p.0 == edge } else { p.1 == edge })
                .map(|p| if horizontal { p.1 } else { p.0 })
                .collect();
            let middle = (across.iter().min().copied().unwrap_or(0)
                + across.iter().max().copied().unwrap_or(0))
                / 2;
            let (ex, ey) = if horizontal {
                (edge, middle)
            } else {
                (middle, edge)
            };
            let outward = |x, y| {
                if horizontal {
                    if end { x > maxx } else { x < minx }
                } else if end {
                    y > maxy
                } else {
                    y < miny
                }
            };
            if (1..h - 1).any(|y| {
                (1..w - 1).any(|x| {
                    outward(x, y)
                        && x.abs_diff(ex).max(y.abs_diff(ey)) <= 3
                        && TILES[tiles[index(x, y)] as usize].look >> 4 == 15
                })
            }) {
                continue;
            }
            let choice = (1..h - 1)
                .flat_map(|y| (1..w - 1).map(move |x| (x, y)))
                .filter(|&(x, y)| {
                    let t = &TILES[tiles[index(x, y)] as usize];
                    let mt = &TILES[tiles[total - 1 - index(x, y)] as usize];
                    let distance = x.abs_diff(ex).max(y.abs_diff(ey));
                    outward(x, y)
                        && distance <= 3
                        && (if horizontal { y != ey } else { x != ex })
                        && (t.passable || (t.water && distance <= 2))
                        && t.look >> 4 != 8
                        && (t.water || stays_connected(x, y, tiles))
                        && t.ore == 0
                        && (mt.passable || mt.water)
                        && mt.look >> 4 != 8
                        && mt.ore == 0
                        && [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]
                            .iter()
                            .filter(|&&(a, b)| {
                                let n = &TILES[tiles[index(a, b)] as usize];
                                n.passable || (t.water && n.water)
                            })
                            .count()
                            >= 3
                })
                // Some original decks are one-bank piers: the seaward end gets a raised hut
                // foundation beside the deck, without creating any new traversable land.
                .min_by_key(|&(x, y)| {
                    (
                        TILES[tiles[index(x, y)] as usize].water,
                        (x - ex).pow(2) + (y - ey).pow(2),
                        y,
                        x,
                    )
                });
            if let Some((x, y)) = choice {
                let i = index(x, y);
                let hut = if TILES[tiles[i] as usize].water {
                    56
                } else {
                    55
                };
                tiles[i] = hut;
                tiles[total - 1 - i] = hut;
            }
        }
    }
}

/// A painted map's tiles (row-major; empty for rule maps), painted once and only when asked
/// for, so that `gen_maps preview <dir> <id>` or `check <id>` paints that map alone.
fn painted(index: usize) -> &'static [u32] {
    static PAINTED: [std::sync::OnceLock<Vec<u32>>; MAPS.len()] =
        [const { std::sync::OnceLock::new() }; MAPS.len()];
    PAINTED[index].get_or_init(|| {
        let spec = &MAPS[index];
        match spec.terrain {
            Terrain::Rule(_) => Vec::new(),
            Terrain::Painted => {
                let (_, biome, recipe) = world_maps::RECIPES
                    .iter()
                    .find(|(id, _, _)| *id == spec.id)
                    .unwrap_or_else(|| panic!("no recipe for {}", spec.id));
                let mut painter =
                    world_maps::Painter::new(spec.width, spec.height, &spec.starts(), *biome);
                recipe(&mut painter);
                neutral_sites(spec, &mut painter.tiles);
                painter.tiles
            }
        }
    })
}

impl MapSpec {
    /// A painted map whose size and homes are given beside its recipe.
    const fn painted(id: &'static str, layout: world_maps::Layout) -> MapSpec {
        MapSpec {
            id,
            width: layout.width,
            height: layout.height,
            homes: layout.homes,
            terrain: Terrain::Painted,
        }
    }

    /// The start positions: every home followed by its point mirror (the yard is 3 x 3 cells and
    /// a start position is its origin, so the mirror of (x, y) is (W - 3 - x, H - 3 - y)).
    pub fn starts(&self) -> Vec<(u32, u32)> {
        self.homes
            .iter()
            .flat_map(|&(x, y)| [(x, y), (self.width - 3 - x, self.height - 3 - y)])
            .collect()
    }

    /// The tile id of cell (x, y).
    pub fn tile_at(&self, x: u32, y: u32) -> u32 {
        let rule = match self.terrain {
            Terrain::Rule(rule) => rule,
            Terrain::Painted => {
                let index = MAPS
                    .iter()
                    .position(|spec| spec.id == self.id)
                    .expect("a shipped map");
                return painted(index)[(y * self.width + x) as usize];
            }
        };
        // Point symmetry: the second half of the cells takes the terrain of its mirror cell, so
        // that the rule below needs to be right for one half only and both starts are equal.
        let (w, h) = (self.width as i32, self.height as i32);
        let (x, y) = (x as i32, y as i32);
        let (mx, my) = (w - 1 - x, h - 1 - y);
        let (x, y) = if (y, x) <= (my, mx) { (x, y) } else { (mx, my) };
        let base = rule(x, y);
        if base != GRASS {
            return base;
        }
        match battlefield_layout::detail(self.width, self.height, x as u32, y as u32) {
            battlefield_layout::Detail::Road => ROAD,
            battlefield_layout::Detail::Grove => FOREST,
            battlefield_layout::Detail::Village => COTTAGE,
            _ => base,
        }
    }

    /// The rules of `tests/m7_maps.rs` that need no files, on the generator's own tiles: point
    /// symmetry, a base site and nine ore cells within 10 cells of every start, starts at least
    /// 12 cells apart, and every passable cell reachable by land from the first start. Returns a
    /// summary of the map's ground and the rules it breaks (at most a few of each kind).
    pub fn check(&self) -> (String, Vec<String>) {
        let (w, h) = (self.width as i32, self.height as i32);
        let tile = |x: i32, y: i32| &TILES[self.tile_at(x as u32, y as u32) as usize];
        let inside = |x: i32, y: i32| (0..w).contains(&x) && (0..h).contains(&y);
        let mut problems = Vec::new();
        let mut asymmetric = 0;
        for y in 0..h {
            for x in 0..w {
                if self.tile_at(x as u32, y as u32)
                    != self.tile_at((w - 1 - x) as u32, (h - 1 - y) as u32)
                {
                    asymmetric += 1;
                }
            }
        }
        if asymmetric > 0 {
            problems.push(format!("{asymmetric} cells differ from their point mirror"));
        }
        if w > 256 || h > 256 {
            problems.push(format!("{w} x {h} is over 256 cells a side"));
        }
        let starts: Vec<(i32, i32)> = self
            .starts()
            .iter()
            .map(|&(x, y)| (x as i32, y as i32))
            .collect();
        for &(sx, sy) in &starts {
            let blocked = (sy - 1..=sy + 3)
                .flat_map(|y| (sx - 1..=sx + 3).map(move |x| (x, y)))
                .filter(|&(x, y)| !inside(x, y) || !tile(x, y).buildable)
                .count();
            if blocked > 0 {
                problems.push(format!(
                    "start ({sx}, {sy}): {blocked} of the 25 yard cells not buildable"
                ));
            }
            let ore = (sy - 10..=sy + 10)
                .flat_map(|y| (sx - 10..=sx + 10).map(move |x| (x, y)))
                .filter(|&(x, y)| inside(x, y) && tile(x, y).ore > 0)
                .count();
            if ore < 9 {
                problems.push(format!(
                    "start ({sx}, {sy}): {ore} ore cells within 10 (needs 9)"
                ));
            }
        }
        for (i, a) in starts.iter().enumerate() {
            for b in &starts[i + 1..] {
                if (a.0 - b.0).abs().max((a.1 - b.1).abs()) < 12 {
                    problems.push(format!("starts {a:?} and {b:?} are under 12 cells apart"));
                }
            }
        }
        // Land reachable from the first start (four-neighbour steps, as the test).
        let index = |x: i32, y: i32| (y * w + x) as usize;
        let mut seen = vec![false; (w * h) as usize];
        let (fx, fy) = starts[0];
        let mut queue = std::collections::VecDeque::from([(fx, fy)]);
        seen[index(fx, fy)] = true;
        while let Some((x, y)) = queue.pop_front() {
            for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                if inside(nx, ny) && tile(nx, ny).passable && !seen[index(nx, ny)] {
                    seen[index(nx, ny)] = true;
                    queue.push_back((nx, ny));
                }
            }
        }
        for &(sx, sy) in &starts[1..] {
            if !seen[index(sx, sy)] {
                problems.push(format!(
                    "start ({sx}, {sy}) has no land path from the first"
                ));
            }
        }
        let cells: Vec<(i32, i32)> = (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).collect();
        let cut_off: Vec<_> = cells
            .iter()
            .filter(|&&(x, y)| tile(x, y).passable && !seen[index(x, y)])
            .collect();
        if let Some(first) = cut_off.first() {
            problems.push(format!(
                "{} passable cells cut off from the first start, e.g. {first:?}",
                cut_off.len()
            ));
        }
        let share = |f: &dyn Fn(&TileKind) -> bool| {
            cells.iter().filter(|&&(x, y)| f(tile(x, y))).count() * 100 / cells.len()
        };
        let farthest = starts
            .iter()
            .skip(1)
            .map(|&(x, y)| (((x - fx).pow(2) + (y - fy).pow(2)) as f64).sqrt() as i32)
            .max()
            .unwrap_or(0);
        let summary = format!(
            "{w} x {h}, {} starts; buildable {}%, passable {}%, water {}%, ore cells {}; \
             first pair {} cells apart (farthest from the first: {farthest})",
            starts.len(),
            share(&|t| t.buildable),
            share(&|t| t.passable),
            share(&|t| t.water),
            cells.iter().filter(|&&(x, y)| tile(x, y).ore > 0).count(),
            (((starts[1].0 - fx).pow(2) + (starts[1].1 - fy).pow(2)) as f64).sqrt() as i32,
        );
        problems.truncate(12);
        (summary, problems)
    }

    pub fn output_dir(&self) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/maps")
            .join(self.id)
    }

    pub fn tileset_name(&self) -> String {
        format!("{}_tiles.png", self.id)
    }
}

fn distance2(x: i32, y: i32, cx: i32, cy: i32) -> i32 {
    (x - cx) * (x - cx) + (y - cy) * (y - cy)
}

fn inside(x: i32, y: i32, x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
    (x0..=x1).contains(&x) && (y0..=y1).contains(&y)
}

/// Crossroads, 56 × 56. Starts at (9, 9) and (44, 44); the rule covers every cell, and
/// [`MapSpec::tile_at`] evaluates it for the cells of the first half only.
fn crossroads(x: i32, y: i32) -> u32 {
    // Lakes off the two empty corners, with sand shores.
    let lake = distance2(x, y, 42, 12);
    if lake <= 14 {
        return WATER;
    }
    if lake <= 26 {
        return SAND;
    }
    let lake = distance2(x, y, 13, 42);
    if lake <= 14 {
        return WATER;
    }
    if lake <= 26 {
        return SAND;
    }
    // Rock ridges that leave three lanes: along both main diagonals of the centre, broken in the
    // middle, and short spurs that shelter the bases.
    if (x - y).abs() <= 1 && ((18..=24).contains(&x) || (31..=37).contains(&x)) {
        return ROCK;
    }
    if inside(x, y, 20, 2, 22, 9) || inside(x, y, 2, 20, 9, 22) {
        return ROCK;
    }
    if inside(x, y, 33, 46, 35, 53) || inside(x, y, 46, 33, 53, 35) {
        return ROCK;
    }
    // Ore: two fields beside each base (off the way to the centre, where its army gathers), one
    // further out on each side, and a contested centre.
    if distance2(x, y, 16, 6) <= 5 || distance2(x, y, 6, 16) <= 5 {
        return ORE;
    }
    if distance2(x, y, 27, 12) <= 4 || distance2(x, y, 12, 27) <= 4 {
        return ORE;
    }
    if distance2(x, y, 28, 43) <= 4 || distance2(x, y, 43, 28) <= 4 {
        return ORE;
    }
    if distance2(2 * x, 2 * y, 55, 55) <= 4 * 10 {
        return ORE;
    }
    GRASS
}

/// Riverlands, 64 × 48. Starts at (8, 21) and (53, 24); a river runs from north to south through
/// the middle (x = 30 to 33) and can be crossed at three fords.
fn riverlands(x: i32, y: i32) -> u32 {
    let ford = (5..=8).contains(&y) || (22..=25).contains(&y) || (39..=42).contains(&y);
    if (30..=33).contains(&x) {
        return if ford { SAND } else { WATER };
    }
    if (x == 29 || x == 34) && !ford {
        return SAND;
    }
    // Rock outcrops that funnel the approaches to the fords.
    if inside(x, y, 20, 12, 23, 17) || inside(x, y, 40, 30, 43, 35) {
        return ROCK;
    }
    if inside(x, y, 20, 30, 22, 36) || inside(x, y, 41, 11, 43, 17) {
        return ROCK;
    }
    // Ore: one field beside each base, one behind it, and contested fields by the middle ford.
    if distance2(x, y, 14, 13) <= 5 || distance2(x, y, 49, 34) <= 5 {
        return ORE;
    }
    if distance2(x, y, 6, 36) <= 4 || distance2(x, y, 57, 11) <= 4 {
        return ORE;
    }
    if distance2(x, y, 26, 28) <= 4 || distance2(x, y, 37, 19) <= 4 {
        return ORE;
    }
    GRASS
}

#[path = "art/terrain.rs"]
mod terrain;

/// RGBA pixels of the tileset, row-major: one 64×32 textured diamond per tile kind, with a darker
/// outline.
pub fn tileset_pixels() -> (u32, u32, Vec<u8>) {
    let width = TILE_WIDTH * TILES.len() as u32;
    let height = TILE_HEIGHT;
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let tile = x / TILE_WIDTH;
            let local = x % TILE_WIDTH;
            // Diamond test in integer arithmetic on doubled coordinates, measured from the tile
            // centre: |dx| / W + |dy| / H <= 1.
            let dx = (2 * local + 1).abs_diff(TILE_WIDTH);
            let dy = (2 * y + 1).abs_diff(TILE_HEIGHT);
            let distance = dx * TILE_HEIGHT + dy * TILE_WIDTH;
            let limit = TILE_WIDTH * TILE_HEIGHT;
            if distance > limit {
                pixels.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            let colour = if tile < 5 {
                terrain::pixel(tile, local, y)
            } else {
                // Later kinds: their base colour with a light fixed grain.
                let grain = ((local * 7 + y * 13 + tile * 5) % 9) as i32 - 4;
                TILES[tile as usize]
                    .colour
                    .map(|c| (i32::from(c) + grain).clamp(0, 255) as u8)
            };
            pixels.extend_from_slice(&colour);
            pixels.push(255);
        }
    }
    (width, height, pixels)
}

pub fn tileset_png() -> Vec<u8> {
    let (width, height, pixels) = tileset_pixels();
    spike_tileset::encode_png(width, height, &pixels)
}

/// The map file of `spec`, in the supported subset of the Tiled format (docs/DECISIONS.md D13).
pub fn map_tmx(spec: &MapSpec) -> String {
    let mut tmx = String::new();
    let tile_count = TILES.len();
    let image_width = TILE_WIDTH as usize * tile_count;
    let (width, height) = (spec.width, spec.height);
    let _ = writeln!(tmx, r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    let _ = writeln!(
        tmx,
        "<!-- Skirmish map {}, generated by `cargo run -p game --example gen_maps`; do not edit by hand. -->",
        spec.id
    );
    let _ = writeln!(
        tmx,
        r#"<map version="1.10" tiledversion="1.11.2" orientation="isometric" renderorder="right-down" width="{width}" height="{height}" tilewidth="{TILE_WIDTH}" tileheight="{TILE_HEIGHT}" infinite="0" nextlayerid="2" nextobjectid="1">"#
    );
    let _ = writeln!(tmx, " <properties>");
    let _ = writeln!(
        tmx,
        r#"  <property name="schema_version" type="int" value="2"/>"#
    );
    let _ = writeln!(tmx, " </properties>");
    let _ = writeln!(
        tmx,
        r#" <tileset firstgid="1" name="{}_tiles" tilewidth="{TILE_WIDTH}" tileheight="{TILE_HEIGHT}" tilecount="{tile_count}" columns="{tile_count}">"#,
        spec.id
    );
    let _ = writeln!(
        tmx,
        r#"  <image source="{}" width="{image_width}" height="{TILE_HEIGHT}"/>"#,
        spec.tileset_name()
    );
    for (id, tile) in TILES.iter().enumerate() {
        let _ = writeln!(tmx, "  <!-- {} -->", tile.name);
        let _ = writeln!(tmx, r#"  <tile id="{id}">"#);
        let _ = writeln!(tmx, "   <properties>");
        if tile.water {
            let _ = writeln!(
                tmx,
                r#"    <property name="water" type="bool" value="true"/>"#
            );
        }
        let _ = writeln!(
            tmx,
            r#"    <property name="passable" type="bool" value="{}"/>"#,
            tile.passable
        );
        let _ = writeln!(
            tmx,
            r#"    <property name="buildable" type="bool" value="{}"/>"#,
            tile.buildable
        );
        let _ = writeln!(
            tmx,
            r#"    <property name="ore" type="int" value="{}"/>"#,
            tile.ore
        );
        let _ = writeln!(
            tmx,
            r#"    <property name="look" type="int" value="{}"/>"#,
            tile.look
        );
        let _ = writeln!(tmx, "   </properties>");
        let _ = writeln!(tmx, "  </tile>");
    }
    let _ = writeln!(tmx, " </tileset>");
    let _ = writeln!(
        tmx,
        r#" <layer id="1" name="terrain" width="{width}" height="{height}">"#
    );
    let _ = writeln!(tmx, r#"  <data encoding="csv">"#);
    let rows: Vec<String> = (0..height)
        .map(|y| {
            (0..width)
                .map(|x| (spec.tile_at(x, y) + 1).to_string())
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect();
    let _ = writeln!(tmx, "{}", rows.join(",\n"));
    let _ = writeln!(tmx, "</data>");
    let _ = writeln!(tmx, " </layer>");
    let _ = writeln!(tmx, "</map>");
    tmx
}

/// A picture of `spec` for reviewing a recipe: `scale` pixels a cell in the tiles' colours, the
/// yards in magenta, and a tick every 32 cells along the top and left edges.
pub fn preview_png(spec: &MapSpec, scale: u32) -> Vec<u8> {
    let (width, height) = (spec.width * scale, spec.height * scale);
    let starts = spec.starts();
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for py in 0..height {
        for px in 0..width {
            let (x, y) = (px / scale, py / scale);
            let yard = starts
                .iter()
                .any(|&(sx, sy)| (sx..sx + 3).contains(&x) && (sy..sy + 3).contains(&y));
            let tick = (x % 32 == 0 && y < 3) || (y % 32 == 0 && x < 3);
            let colour = if yard || tick {
                [255, 0, 255]
            } else {
                TILES[spec.tile_at(x, y) as usize].colour
            };
            pixels.extend_from_slice(&colour);
            pixels.push(255);
        }
    }
    spike_tileset::encode_png(width, height, &pixels)
}

/// `gen_maps` writes the maps; `gen_maps preview <dir> [id ...]` writes pictures of them (all, or
/// those named) to `<dir>` instead; `gen_maps starts` prints the map list of `skirmish.ron`.
fn main() -> std::io::Result<()> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match arguments.first().map(String::as_str) {
        Some("preview") => {
            let dir = PathBuf::from(arguments.get(1).map_or(".", String::as_str));
            std::fs::create_dir_all(&dir)?;
            for spec in &MAPS {
                if arguments.len() > 2 && !arguments[2..].iter().any(|id| id == spec.id) {
                    continue;
                }
                let path = dir.join(format!("{}.png", spec.id));
                std::fs::write(&path, preview_png(spec, 3))?;
                println!("wrote {}", path.display());
            }
            return Ok(());
        }
        Some("check") => {
            let mut failed = false;
            for spec in &MAPS {
                if arguments.len() > 1 && !arguments[1..].iter().any(|id| id == spec.id) {
                    continue;
                }
                let (summary, problems) = spec.check();
                println!("{}: {summary}", spec.id);
                for problem in &problems {
                    println!("  {problem}");
                }
                failed |= !problems.is_empty();
            }
            if failed {
                std::process::exit(1);
            }
            return Ok(());
        }
        Some("starts") => {
            for spec in &MAPS {
                let starts: Vec<String> = spec
                    .starts()
                    .iter()
                    .map(|(x, y)| format!("(x: {x}, y: {y})"))
                    .collect();
                println!(
                    "        (id: \"{}\", starts: [{}]),",
                    spec.id,
                    starts.join(", ")
                );
            }
            return Ok(());
        }
        _ => {}
    }

    for spec in &MAPS {
        let dir = spec.output_dir();
        std::fs::create_dir_all(&dir)?;
        for (name, contents) in [
            (format!("{}.tmx", spec.id), map_tmx(spec).into_bytes()),
            (spec.tileset_name(), tileset_png()),
        ] {
            let path = dir.join(name);
            std::fs::write(&path, contents)?;
            println!("wrote {}", path.display());
        }
    }
    Ok(())
}
