//! Generates the M1 demo map: `assets/maps/demo/demo.tmx` (64×64 cells) and its placeholder
//! tileset `assets/maps/demo/demo_tiles.png`.
//!
//! Run with `cargo run -p game --example gen_demo_map`. The output is deterministic, and
//! `crates/game/tests/m1_demo_map.rs` checks that the committed files match it byte for byte.
//!
//! The map shows every terrain kind: grass, ore fields, a lake with a sand shore, and a rock
//! ridge along the diagonal with a gap. Three corner cells carry marker tiles so that the
//! orientation of the rendered map can be checked by eye: red at cell (0, 0), magenta at
//! (63, 0), and white at (0, 63).

#[path = "art/battlefield_layout.rs"]
mod battlefield_layout;
#[path = "art/looks.rs"]
#[allow(dead_code)]
mod looks;
use looks::{Feature as F, Ground as G, look};
use std::fmt::Write as _;
use std::path::PathBuf;

#[path = "gen_spike_tileset.rs"]
#[allow(dead_code)]
mod spike_tileset;

#[path = "art/terrain.rs"]
mod terrain;

pub const SIDE: u32 = 64;
pub const TILE_WIDTH: u32 = 64;
pub const TILE_HEIGHT: u32 = 32;

/// A tile of the demo tileset: name, colour, and the terrain properties of the map format
/// (docs/DECISIONS.md, D8).
pub struct TileKind {
    pub name: &'static str,
    pub colour: [u8; 3],
    pub passable: bool,
    pub buildable: bool,
    pub ore: i32,
    /// The presentation-only look (`art/looks.rs`).
    pub look: u8,
}

const fn kind(
    name: &'static str,
    colour: [u8; 3],
    passable: bool,
    buildable: bool,
    ore: i32,
    look: u8,
) -> TileKind {
    TileKind {
        name,
        colour,
        passable,
        buildable,
        ore,
        look,
    }
}

/// The tileset, indexed by tile id.
pub const TILES: [TileKind; 11] = [
    kind(
        "grass",
        [76, 140, 60],
        true,
        true,
        0,
        look(G::Grass, F::None),
    ),
    kind(
        "ore",
        [200, 160, 40],
        true,
        false,
        500,
        look(G::Grass, F::Ore),
    ),
    kind(
        "water",
        [40, 90, 170],
        false,
        false,
        0,
        look(G::Water, F::None),
    ),
    kind(
        "rock",
        [110, 105, 100],
        false,
        false,
        0,
        look(G::Grass, F::Cliff),
    ),
    kind(
        "sand",
        [190, 175, 120],
        true,
        false,
        0,
        look(G::Sand, F::None),
    ),
    kind(
        "marker (0, 0)",
        [210, 50, 50],
        true,
        true,
        0,
        look(G::Grass, F::None),
    ),
    kind(
        "marker (63, 0)",
        [200, 60, 200],
        true,
        true,
        0,
        look(G::Grass, F::None),
    ),
    kind(
        "marker (0, 63)",
        [235, 235, 235],
        true,
        true,
        0,
        look(G::Grass, F::None),
    ),
    kind(
        "road",
        [86, 86, 84],
        true,
        false,
        0,
        look(G::Grass, F::Road),
    ),
    kind(
        "forest",
        [40, 84, 40],
        false,
        false,
        0,
        look(G::Grass, F::Forest),
    ),
    kind(
        "cottage",
        [150, 104, 84],
        false,
        false,
        0,
        look(G::Grass, F::House),
    ),
];

pub const GRASS: u32 = 0;
pub const ORE: u32 = 1;
pub const WATER: u32 = 2;
pub const ROCK: u32 = 3;
pub const SAND: u32 = 4;
pub const MARKER_ORIGIN: u32 = 5;
pub const MARKER_X_END: u32 = 6;
pub const MARKER_Y_END: u32 = 7;
pub const ROAD: u32 = 8;
pub const FOREST: u32 = 9;
pub const COTTAGE: u32 = 10;

pub fn output_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/maps/demo")
}

/// Tile id of cell (x, y).
pub fn tile_at(x: u32, y: u32) -> u32 {
    let last = SIDE - 1;
    match (x, y) {
        (0, 0) => return MARKER_ORIGIN,
        (x, 0) if x == last => return MARKER_X_END,
        (0, y) if y == last => return MARKER_Y_END,
        _ => {}
    }
    let (x, y) = (x as i32, y as i32);
    let distance2 = |cx: i32, cy: i32| (x - cx).pow(2) + (y - cy).pow(2);
    if distance2(40, 22) <= 36 {
        WATER
    } else if distance2(40, 22) <= 56 {
        SAND
    } else if (x - y).abs() <= 1 && (16..=48).contains(&x) && !(30..=33).contains(&x) {
        ROCK
    } else if distance2(12, 12) <= 8 || distance2(51, 51) <= 8 || distance2(22, 44) <= 5 {
        ORE
    } else {
        match battlefield_layout::detail(SIDE, SIDE, x as u32, y as u32) {
            battlefield_layout::Detail::Road => ROAD,
            battlefield_layout::Detail::Grove => FOREST,
            battlefield_layout::Detail::Village => COTTAGE,
            _ => GRASS,
        }
    }
}

/// RGBA pixels of the tileset, row-major: one 64×32 diamond per tile kind, with a darker
/// outline so that individual cells are distinguishable.
pub fn tileset_pixels() -> (u32, u32, Vec<u8>) {
    let width = TILE_WIDTH * TILES.len() as u32;
    let height = TILE_HEIGHT;
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let colour = TILES[(x / TILE_WIDTH) as usize].colour;
            // Diamond test in integer arithmetic on doubled coordinates, measured from the
            // tile centre: |dx| / W + |dy| / H <= 1.
            let dx = (2 * (x % TILE_WIDTH) + 1).abs_diff(TILE_WIDTH);
            let dy = (2 * y + 1).abs_diff(TILE_HEIGHT);
            let distance = dx * TILE_HEIGHT + dy * TILE_WIDTH;
            let limit = TILE_WIDTH * TILE_HEIGHT;
            if distance > limit {
                pixels.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                let tile = x / TILE_WIDTH;
                let colour = if tile < 5 {
                    terrain::pixel(tile, x % TILE_WIDTH, y)
                } else {
                    colour
                };
                pixels.extend_from_slice(&colour);
                pixels.push(255);
            }
        }
    }
    (width, height, pixels)
}

pub fn tileset_png() -> Vec<u8> {
    let (width, height, pixels) = tileset_pixels();
    spike_tileset::encode_png(width, height, &pixels)
}

pub fn map_tmx() -> String {
    let mut tmx = String::new();
    let tile_count = TILES.len();
    let image_width = TILE_WIDTH as usize * tile_count;
    let _ = writeln!(tmx, r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    let _ = writeln!(
        tmx,
        "<!-- M1 demo map, generated by `cargo run -p game --example gen_demo_map`; do not edit by hand. -->"
    );
    let _ = writeln!(
        tmx,
        r#"<map version="1.10" tiledversion="1.11.2" orientation="isometric" renderorder="right-down" width="{SIDE}" height="{SIDE}" tilewidth="{TILE_WIDTH}" tileheight="{TILE_HEIGHT}" infinite="0" nextlayerid="2" nextobjectid="1">"#
    );
    let _ = writeln!(tmx, " <properties>");
    let _ = writeln!(
        tmx,
        r#"  <property name="schema_version" type="int" value="2"/>"#
    );
    let _ = writeln!(tmx, " </properties>");
    let _ = writeln!(
        tmx,
        r#" <tileset firstgid="1" name="demo_tiles" tilewidth="{TILE_WIDTH}" tileheight="{TILE_HEIGHT}" tilecount="{tile_count}" columns="{tile_count}">"#
    );
    let _ = writeln!(
        tmx,
        r#"  <image source="demo_tiles.png" width="{image_width}" height="{TILE_HEIGHT}"/>"#
    );
    for (id, tile) in TILES.iter().enumerate() {
        let _ = writeln!(tmx, "  <!-- {} -->", tile.name);
        let _ = writeln!(tmx, r#"  <tile id="{id}">"#);
        let _ = writeln!(tmx, "   <properties>");
        if tile.name == "water" {
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
        r#" <layer id="1" name="terrain" width="{SIDE}" height="{SIDE}">"#
    );
    let _ = writeln!(tmx, r#"  <data encoding="csv">"#);
    let rows: Vec<String> = (0..SIDE)
        .map(|y| {
            (0..SIDE)
                .map(|x| (tile_at(x, y) + 1).to_string())
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

fn main() -> std::io::Result<()> {
    let dir = output_dir();
    std::fs::create_dir_all(&dir)?;
    for (name, contents) in [
        ("demo.tmx", map_tmx().into_bytes()),
        ("demo_tiles.png", tileset_png()),
    ] {
        let path = dir.join(name);
        std::fs::write(&path, contents)?;
        println!("wrote {}", path.display());
    }
    Ok(())
}
