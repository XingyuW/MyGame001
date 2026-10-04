//! Generates `assets/maps/spike/spike_tiles.png`, the placeholder tileset of the M0 spike map.
//!
//! Run with `cargo run -p game --example gen_spike_tileset`. The output is deterministic, and
//! `crates/game/tests/m0_spike.rs` checks that the committed file matches it byte for byte.
//!
//! Four 64x32 isometric diamonds in one row: grass, ore, a red marker for cell (0, 0), and a
//! blue marker for the far end of the x axis.

use std::path::PathBuf;

pub const TILE_WIDTH: u32 = 64;
pub const TILE_HEIGHT: u32 = 32;
pub const TILE_COLOURS: [[u8; 3]; 4] =
    [[76, 140, 60], [200, 160, 40], [210, 50, 50], [50, 90, 210]];

pub fn output_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/maps/spike/spike_tiles.png")
}

/// RGBA pixels of the tileset, row-major.
pub fn tileset_pixels() -> (u32, u32, Vec<u8>) {
    let width = TILE_WIDTH * TILE_COLOURS.len() as u32;
    let height = TILE_HEIGHT;
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let colour = TILE_COLOURS[(x / TILE_WIDTH) as usize];
            // Diamond test in integer arithmetic on doubled coordinates, measured from the
            // tile centre: |dx| / W + |dy| / H <= 1.
            let dx = (2 * (x % TILE_WIDTH) + 1).abs_diff(TILE_WIDTH);
            let dy = (2 * y + 1).abs_diff(TILE_HEIGHT);
            let distance = dx * TILE_HEIGHT + dy * TILE_WIDTH;
            let limit = TILE_WIDTH * TILE_HEIGHT;
            if distance > limit {
                pixels.extend_from_slice(&[0, 0, 0, 0]);
            } else if distance > limit - 2 * TILE_WIDTH {
                // Darker outline so individual cells are distinguishable.
                pixels.extend(colour.iter().map(|c| c / 2));
                pixels.push(255);
            } else {
                pixels.extend_from_slice(&colour);
                pixels.push(255);
            }
        }
    }
    (width, height, pixels)
}

/// Minimal PNG encoder: 8-bit RGBA, no filtering, stored (uncompressed) deflate blocks.
pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(rgba.len() + height as usize);
    for row in rgba.chunks_exact((width * 4) as usize) {
        raw.push(0); // filter type: none
        raw.extend_from_slice(row);
    }

    let mut zlib = vec![0x78, 0x01];
    let mut blocks = raw.chunks(0xffff).peekable();
    while let Some(block) = blocks.next() {
        zlib.push(u8::from(blocks.peek().is_none()));
        let len = block.len() as u16;
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&(!len).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit depth, RGBA, deflate, no filter, no interlace

    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    write_chunk(&mut png, b"IHDR", &ihdr);
    write_chunk(&mut png, b"IDAT", &zlib);
    write_chunk(&mut png, b"IEND", &[]);
    png
}

pub fn tileset_png() -> Vec<u8> {
    let (width, height, pixels) = tileset_pixels();
    encode_png(width, height, &pixels)
}

fn write_chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = png.len();
    png.extend_from_slice(kind);
    png.extend_from_slice(data);
    let crc = crc32(&png[start..]);
    png.extend_from_slice(&crc.to_be_bytes());
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1_u32, 0_u32);
    for &byte in bytes {
        a = (a + u32::from(byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

fn main() -> std::io::Result<()> {
    let path = output_path();
    std::fs::write(&path, tileset_png())?;
    println!("wrote {}", path.display());
    Ok(())
}
