//! Generates the M7 skirmish maps `assets/maps/<id>/<id>.tmx` with their tilesets
//! `assets/maps/<id>/<id>_tiles.png` (docs/DECISIONS.md D50).
//!
//! Run with `cargo run -p game --example gen_maps`. The output is deterministic (integer
//! arithmetic only), and `crates/game/tests/m7_maps.rs` checks that the committed files match it
//! byte for byte.
//!
//! Both maps are point-symmetric about their centre, so that the two start positions are equal:
//! the cell (x, y) has the terrain of (W − 1 − x, H − 1 − y). Each start has buildable ground for
//! a base, an ore field nearby, and a second field further out; contested ore lies in the middle.
//!
//! * `crossroads` (56 × 56): starts in opposite corners, rock ridges and two lakes that leave
//!   three lanes across the map.
//! * `riverlands` (64 × 48): starts on the west and east banks of a river that can be crossed at
//!   three fords.
//!
//! The tileset is textured: every tile kind carries a fixed pattern of lighter and darker pixels
//! from an integer hash, and an outline, so that cells stay distinguishable.

use std::fmt::Write as _;
use std::path::PathBuf;

#[path = "gen_spike_tileset.rs"]
#[allow(dead_code)]
mod spike_tileset;

pub const TILE_WIDTH: u32 = 64;
pub const TILE_HEIGHT: u32 = 32;

/// A tile of the tileset: name, base colour, and the terrain properties of the map format
/// (docs/DECISIONS.md, D8).
pub struct TileKind {
    pub name: &'static str,
    pub colour: [u8; 3],
    pub passable: bool,
    pub buildable: bool,
    pub ore: i32,
}

const fn kind(
    name: &'static str,
    colour: [u8; 3],
    passable: bool,
    buildable: bool,
    ore: i32,
) -> TileKind {
    TileKind {
        name,
        colour,
        passable,
        buildable,
        ore,
    }
}

/// The tileset, indexed by tile id.
pub const TILES: [TileKind; 5] = [
    kind("grass", [74, 132, 58], true, true, 0),
    kind("ore", [196, 158, 44], true, false, 500),
    kind("water", [38, 86, 160], false, false, 0),
    kind("rock", [112, 104, 96], false, false, 0),
    kind("sand", [188, 172, 118], true, false, 0),
];

pub const GRASS: u32 = 0;
pub const ORE: u32 = 1;
pub const WATER: u32 = 2;
pub const ROCK: u32 = 3;
pub const SAND: u32 = 4;

/// A generated map: its id, size, and start positions (as listed in `assets/data/skirmish.ron`).
pub struct MapSpec {
    pub id: &'static str,
    pub width: u32,
    pub height: u32,
    pub starts: [(u32, u32); 2],
    terrain: fn(i32, i32) -> u32,
}

pub const MAPS: [MapSpec; 2] = [
    MapSpec {
        id: "crossroads",
        width: 56,
        height: 56,
        starts: [(9, 9), (44, 44)],
        terrain: crossroads,
    },
    MapSpec {
        id: "riverlands",
        width: 64,
        height: 48,
        starts: [(8, 21), (53, 24)],
        terrain: riverlands,
    },
];

impl MapSpec {
    /// The tile id of cell (x, y).
    pub fn tile_at(&self, x: u32, y: u32) -> u32 {
        // Point symmetry: the second half of the cells takes the terrain of its mirror cell, so
        // that the rule below needs to be right for one half only and both starts are equal.
        let (w, h) = (self.width as i32, self.height as i32);
        let (x, y) = (x as i32, y as i32);
        let (mx, my) = (w - 1 - x, h - 1 - y);
        let (x, y) = if (y, x) <= (my, mx) { (x, y) } else { (mx, my) };
        (self.terrain)(x, y)
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
            let colour = terrain::pixel(tile, local, y);
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
        "<!-- M7 map {}, generated by `cargo run -p game --example gen_maps`; do not edit by hand. -->",
        spec.id
    );
    let _ = writeln!(
        tmx,
        r#"<map version="1.10" tiledversion="1.11.2" orientation="isometric" renderorder="right-down" width="{width}" height="{height}" tilewidth="{TILE_WIDTH}" tileheight="{TILE_HEIGHT}" infinite="0" nextlayerid="2" nextobjectid="1">"#
    );
    let _ = writeln!(tmx, " <properties>");
    let _ = writeln!(
        tmx,
        r#"  <property name="schema_version" type="int" value="1"/>"#
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

fn main() -> std::io::Result<()> {
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
