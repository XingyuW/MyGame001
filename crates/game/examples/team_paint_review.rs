//! Deterministic 1:1 ownership contact sheet through the production texture decoder/palette.
//! Run with `cargo run -p game --example team_paint_review -- /existing/output.png`.
//! Rows: gen_unit_sprites::SPRITES order. Columns: E, SE, S, SW, W, NW, N, NE;
//! each column pairs friendly (left) and enemy (right), at the native 64-pixel frame size.
#[path = "gen_unit_sprites.rs"]
#[allow(dead_code)]
mod gen_unit_sprites;
#[path = "../src/presentation.rs"]
mod presentation;
#[path = "../src/tileset_image.rs"]
#[allow(dead_code)]
mod tileset_image;

use bevy::prelude::*;
use gen_unit_sprites::{FACINGS, FRAME, SPRITES};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args()
        .nth(1)
        .ok_or("provide an output PNG path")?;
    let width = FRAME * FACINGS * 2;
    let height = FRAME * SPRITES.len() as u32;
    let mut pixels = vec![0; (width * height * 4) as usize];
    for (row, kind) in SPRITES.iter().enumerate() {
        let path = gen_unit_sprites::output_dir().join(format!("{}.png", kind.name));
        let bytes = tileset_image::check_png_file(&path, FRAME * FACINGS, FRAME)?;
        let mut images = Assets::<Image>::default();
        let (friendly, enemy) = presentation::sprite_pair(&bytes, &mut images)?;
        for (team, handle) in [friendly, enemy].iter().enumerate() {
            let source = images.get(handle).ok_or("missing image")?;
            let source = source.data.as_ref().ok_or("missing pixels")?;
            for facing in 0..FACINGS {
                for y in 0..FRAME {
                    for x in 0..FRAME {
                        let from = ((y * FRAME * FACINGS + facing * FRAME + x) * 4) as usize;
                        let to = (((row as u32 * FRAME + y) * width
                            + (facing * 2 + team as u32) * FRAME
                            + x)
                            * 4) as usize;
                        let alpha = u32::from(source[from + 3]);
                        for (channel, background) in [65, 73, 53].into_iter().enumerate() {
                            pixels[to + channel] = ((u32::from(source[from + channel]) * alpha
                                + background * (255 - alpha))
                                / 255) as u8;
                        }
                        pixels[to + 3] = 255;
                    }
                }
            }
        }
    }
    std::fs::write(
        output,
        gen_unit_sprites::spike_tileset::encode_png(width, height, &pixels),
    )?;
    Ok(())
}
