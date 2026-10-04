//! Generates the placeholder unit sprite sheets `assets/sprites/units/<sprite>.png` (M2).
//!
//! Run with `cargo run -p game --example gen_unit_sprites`. The output is deterministic (only
//! IEEE-exact arithmetic: no trigonometry), and `crates/game/tests/m2_unit_sprites.rs` checks
//! that the committed files match it byte for byte.
//!
//! A sheet holds one 64×64 frame per facing, in the order of `sim::Direction::ALL` (east,
//! south-east, south, south-west, west, north-west, north, north-east, where +x is east and +y
//! is south in cell coordinates). Each frame shows the unit standing on the ground plane of the
//! isometric projection (PLAN.md section 5, equation 7) and pointing in its facing: a hull
//! elongated along the facing, darker at the front, with a turret and barrel for the tank, a
//! wedge for the scout, and a broad box for the harvester (M3). The sprites are light grey so
//! that the game can tint them with the owner's colour.

use std::path::PathBuf;

#[path = "gen_spike_tileset.rs"]
#[allow(dead_code)]
mod spike_tileset;

/// Width and height of one frame in pixels.
pub const FRAME: u32 = 64;
/// Number of facings (frames per sheet).
pub const FACINGS: u32 = 8;
/// Pixel (x, y) of the ground point under the unit's centre within a frame.
pub const GROUND: (u32, u32) = (32, 44);
/// Tile size of the projection the sprites are drawn for.
const TILE_WIDTH: f64 = 64.0;
const TILE_HEIGHT: f64 = 32.0;

/// Cell offsets of the eight facings, in the order of `sim::Direction::ALL`.
pub const FACING_DELTAS: [(i32, i32); 8] = [
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
];

/// A placeholder sprite: its name (as named by `sprite` in `assets/data/units.ron`) and shape.
pub struct SpriteKind {
    pub name: &'static str,
    /// Half length and half width of the hull, in cells.
    hull: (f64, f64),
    turret: bool,
    /// Without a turret: a hull that narrows to a wedge at the front (scout) or a box (M3:
    /// harvester).
    wedge: bool,
}

pub const SPRITES: [SpriteKind; 3] = [
    SpriteKind {
        name: "light_tank",
        hull: (0.34, 0.22),
        turret: true,
        wedge: false,
    },
    SpriteKind {
        name: "scout",
        hull: (0.28, 0.14),
        turret: false,
        wedge: true,
    },
    SpriteKind {
        name: "harvester",
        hull: (0.38, 0.26),
        turret: false,
        wedge: false,
    },
];

pub fn output_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/sprites/units")
}

/// Height in pixels by which the turret is drawn above the ground plane.
const TURRET_LIFT: f64 = 6.0;

/// The point (u, v) on the ground plane, in cells relative to the unit's centre, that the
/// pixel (x, y) of a frame shows when lifted by `lift` pixels (inverse of equation 7).
fn ground_point(x: u32, y: u32, lift: f64) -> (f64, f64) {
    let a = (f64::from(x) + 0.5 - f64::from(GROUND.0)) / (TILE_WIDTH / 2.0);
    let b = (f64::from(y) + 0.5 + lift - f64::from(GROUND.1)) / (TILE_HEIGHT / 2.0);
    ((a + b) / 2.0, (b - a) / 2.0)
}

/// RGBA colour of pixel (x, y) of the frame for `facing`, or `None` if transparent.
fn frame_pixel(kind: &SpriteKind, facing: (i32, i32), x: u32, y: u32) -> Option<[u8; 4]> {
    let length = f64::from(facing.0 * facing.0 + facing.1 * facing.1).sqrt();
    let forward = (f64::from(facing.0) / length, f64::from(facing.1) / length);
    let side = (-forward.1, forward.0);
    let local = |(u, v): (f64, f64)| (u * forward.0 + v * forward.1, u * side.0 + v * side.1);
    if kind.turret {
        let (along, across) = local(ground_point(x, y, TURRET_LIFT));
        let barrel = (0.0..=0.5).contains(&along) && across.abs() <= 0.05;
        if barrel {
            return Some([70, 70, 70, 255]);
        }
        if along * along + across * across <= 0.13 * 0.13 {
            return Some([235, 235, 235, 255]);
        }
    }
    let (along, across) = local(ground_point(x, y, 0.0));
    let (half_length, half_width) = kind.hull;
    // The scout's hull narrows to a wedge at the front.
    let width_here = if kind.wedge {
        half_width * (1.0 - (along / half_length).max(0.0))
    } else {
        half_width
    };
    if along.abs() > half_length || across.abs() > width_here {
        return None;
    }
    let edge = along.abs() > half_length - 0.05 || across.abs() > width_here - 0.05;
    let front = along > half_length * 0.45;
    // The front is shaded even on the outline, so that the narrow nose of the scout shows.
    Some(match (edge, front) {
        (_, true) => [140, 140, 140, 255],
        (true, false) => [60, 60, 60, 255],
        (false, false) => [200, 200, 200, 255],
    })
}

/// RGBA pixels of a sheet: the eight frames side by side.
pub fn sheet_pixels(kind: &SpriteKind) -> (u32, u32, Vec<u8>) {
    let width = FRAME * FACINGS;
    let height = FRAME;
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let facing = FACING_DELTAS[(x / FRAME) as usize];
            let colour = frame_pixel(kind, facing, x % FRAME, y).unwrap_or([0, 0, 0, 0]);
            pixels.extend_from_slice(&colour);
        }
    }
    (width, height, pixels)
}

pub fn sheet_png(kind: &SpriteKind) -> Vec<u8> {
    let (width, height, pixels) = sheet_pixels(kind);
    spike_tileset::encode_png(width, height, &pixels)
}

fn main() -> std::io::Result<()> {
    let dir = output_dir();
    std::fs::create_dir_all(&dir)?;
    for kind in &SPRITES {
        let path = dir.join(format!("{}.png", kind.name));
        std::fs::write(&path, sheet_png(kind))?;
        println!("wrote {}", path.display());
    }
    Ok(())
}
