//! Generates the placeholder building sprites `assets/sprites/buildings/<sprite>.png` (M3).
//!
//! Run with `cargo run -p game --example gen_building_sprites`. The output is deterministic (only
//! IEEE-exact arithmetic), and `crates/game/tests/m3_building_sprites.rs` checks that the
//! committed files match it byte for byte.
//!
//! A building of `width` × `height` cells is drawn as a box standing on its footprint: the
//! footprint's diamond (PLAN.md section 5, equation 7, with 64×32 pixel tiles) lifted by
//! [`WALL_PX`] pixels as the roof, and the two walls that face the viewer below it. The image is
//! `(width + height) × 32` pixels wide and `(width + height) × 16 + WALL_PX` pixels high; the
//! footprint's diamond fills its lower part, so the image's centre lies `WALL_PX / 2` pixels above
//! the footprint's centre. The roof carries `stripes` bands so that the building types can be told
//! apart, and the sprites are light grey so that the game can tint them with the owner's colour.

use std::path::PathBuf;

#[path = "gen_spike_tileset.rs"]
#[allow(dead_code)]
mod spike_tileset;

/// Height of the walls in pixels.
pub const WALL_PX: u32 = 32;
/// Tile size of the projection the sprites are drawn for.
const HALF_WIDTH: f64 = 32.0;
const HALF_HEIGHT: f64 = 16.0;

/// A placeholder building sprite: its name (as named by `sprite` in `assets/data/economy.ron`),
/// its footprint in cells, and the number of bands on its roof.
pub struct BuildingSprite {
    pub name: &'static str,
    pub width: u8,
    pub height: u8,
    stripes: u32,
}

pub const SPRITES: [BuildingSprite; 5] = [
    BuildingSprite {
        name: "construction_yard",
        width: 3,
        height: 3,
        stripes: 1,
    },
    BuildingSprite {
        name: "power_plant",
        width: 2,
        height: 2,
        stripes: 2,
    },
    BuildingSprite {
        name: "refinery",
        width: 3,
        height: 3,
        stripes: 3,
    },
    BuildingSprite {
        name: "barracks",
        width: 2,
        height: 2,
        stripes: 0,
    },
    BuildingSprite {
        name: "war_factory",
        width: 3,
        height: 3,
        stripes: 4,
    },
];

/// The size in pixels of the sprite of a building of `width` × `height` cells.
pub fn image_size(width: u8, height: u8) -> (u32, u32) {
    let cells = u32::from(width) + u32::from(height);
    (cells * 32, cells * 16 + WALL_PX)
}

pub fn output_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/sprites/buildings")
}

/// The point (u, v) of the footprint, in cells from its top corner, that the pixel (x, y) shows
/// when the footprint is lifted by `lift` pixels (inverse of equation 7).
fn footprint_point(sprite: &BuildingSprite, x: u32, y: u32, lift: f64) -> (f64, f64) {
    let top_x = f64::from(sprite.height) * HALF_WIDTH;
    let top_y = f64::from(WALL_PX) - lift;
    let a = (f64::from(x) + 0.5 - top_x) / HALF_WIDTH;
    let b = (f64::from(y) + 0.5 - top_y) / HALF_HEIGHT;
    ((a + b) / 2.0, (b - a) / 2.0)
}

fn inside(sprite: &BuildingSprite, (u, v): (f64, f64)) -> bool {
    (0.0..=f64::from(sprite.width)).contains(&u) && (0.0..=f64::from(sprite.height)).contains(&v)
}

/// RGBA colour of pixel (x, y), or `None` if transparent.
fn pixel(sprite: &BuildingSprite, x: u32, y: u32) -> Option<[u8; 4]> {
    let (width, height) = (f64::from(sprite.width), f64::from(sprite.height));
    let roof = footprint_point(sprite, x, y, f64::from(WALL_PX));
    if inside(sprite, roof) {
        let (u, v) = roof;
        let edge = u < 0.06 || v < 0.06 || u > width - 0.06 || v > height - 0.06;
        if edge {
            return Some([90, 90, 90, 255]);
        }
        // Bands across the roof, evenly spaced along u.
        let band = (1..=sprite.stripes).any(|index| {
            let centre = width * f64::from(index) / f64::from(sprite.stripes + 1);
            (u - centre).abs() < 0.12
        });
        return Some(if band {
            [160, 160, 160, 255]
        } else {
            [225, 225, 225, 255]
        });
    }
    // Below the roof, a pixel shows a wall if lowering the roof by some height between 0 and
    // WALL_PX (sampled every half pixel) brings the footprint under it.
    let on_wall = (0..2 * WALL_PX).any(|step| {
        let lift = f64::from(step) / 2.0;
        inside(sprite, footprint_point(sprite, x, y, lift))
    });
    if !on_wall {
        return None;
    }
    // The left wall faces south-west (along the footprint's +v edge) and the right wall faces
    // south-east (along its +u edge); they meet below the footprint's bottom corner, which lies
    // `width` half tiles right of the image's left edge.
    let left = f64::from(x) + 0.5 < width * HALF_WIDTH;
    Some(if left {
        [170, 170, 170, 255]
    } else {
        [130, 130, 130, 255]
    })
}

/// RGBA pixels of a building sprite.
pub fn sprite_pixels(sprite: &BuildingSprite) -> (u32, u32, Vec<u8>) {
    let (width, height) = image_size(sprite.width, sprite.height);
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            pixels.extend_from_slice(&pixel(sprite, x, y).unwrap_or([0, 0, 0, 0]));
        }
    }
    (width, height, pixels)
}

pub fn sprite_png(sprite: &BuildingSprite) -> Vec<u8> {
    let (width, height, pixels) = sprite_pixels(sprite);
    spike_tileset::encode_png(width, height, &pixels)
}

fn main() -> std::io::Result<()> {
    let dir = output_dir();
    std::fs::create_dir_all(&dir)?;
    for sprite in &SPRITES {
        let path = dir.join(format!("{}.png", sprite.name));
        std::fs::write(&path, sprite_png(sprite))?;
        println!("wrote {}", path.display());
    }
    Ok(())
}
