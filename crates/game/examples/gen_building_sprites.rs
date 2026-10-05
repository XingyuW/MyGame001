//! Generates the building sprites `assets/sprites/buildings/<sprite>.png` (M3; the gun turret in
//! M4; the tech center and the rocket tower in M7).
//!
//! Run with `cargo run -p game --example gen_building_sprites`. The output is deterministic (only
//! IEEE-exact arithmetic), and `crates/game/tests/m3_building_sprites.rs` checks that the
//! committed files match it byte for byte.
//!
//! Original industrial miniatures: concrete aprons, metal siding, gantry cranes, tanks,
//! turbines and radar. Footprints and ground anchors retain the M3 geometry contract.

use std::path::PathBuf;

#[path = "gen_spike_tileset.rs"]
#[allow(dead_code)]
mod spike_tileset;

/// Height of the walls in pixels.
pub const WALL_PX: u32 = 32;
/// An industrial building sprite: the name from `assets/data/economy.ron` and its footprint.
pub struct BuildingSprite {
    pub name: &'static str,
    pub width: u8,
    pub height: u8,
}

pub const SPRITES: [BuildingSprite; 8] = [
    BuildingSprite {
        name: "construction_yard",
        width: 3,
        height: 3,
    },
    BuildingSprite {
        name: "power_plant",
        width: 2,
        height: 2,
    },
    BuildingSprite {
        name: "refinery",
        width: 3,
        height: 3,
    },
    BuildingSprite {
        name: "barracks",
        width: 2,
        height: 2,
    },
    BuildingSprite {
        name: "war_factory",
        width: 3,
        height: 3,
    },
    BuildingSprite {
        name: "gun_turret",
        width: 1,
        height: 1,
    },
    BuildingSprite {
        name: "tech_center",
        width: 2,
        height: 2,
    },
    BuildingSprite {
        name: "rocket_tower",
        width: 1,
        height: 1,
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

#[path = "art/painter.rs"]
#[allow(dead_code)]
mod art;
use art::{Canvas, DARK, LAMP, STEEL, TEAM, TRIM, shade};

/// Corrugated industrial building, with a team fascia, windows and roof ventilators.
fn hall(c: &mut Canvas, p: (f64, f64), size: (f64, f64), height: f64) {
    let (u, v) = p;
    let (w, d) = size;
    c.block((u, v, 2.0), (w, d, height), STEEL);
    c.beam((u, v + d, 3.0), (u + w, v + d, 3.0), DARK);
    c.beam((u + w, v, 3.0), (u + w, v + d, 3.0), DARK);
    c.beam((u, v + d, 4.0), (u + w, v + d, 4.0), shade(STEEL, -49));
    // Roof panel joins, cap bolts and small streaks give the metal a worked surface.

    for i in 1..(w * 9.0) as i32 {
        let x = u + f64::from(i) / 9.0;
        c.beam((x, v + d, 3.0), (x, v + d, height), shade(STEEL, -35));
    }
    for i in 1..(d * 9.0) as i32 {
        let y = v + f64::from(i) / 9.0;
        c.beam((u + w, y, 3.0), (u + w, y, height), shade(STEEL, -57));
    }
    c.block((u, v, height - 2.0), (w, d, 3.0), TEAM);
    c.block(
        (u + 0.12, v + 0.12, height + 2.0),
        (w - 0.24, d - 0.24, 2.0),
        [79, 91, 94],
    );
    for i in 1..(w * 5.0) as i32 {
        let x = u + f64::from(i) * 0.2;
        c.beam(
            (x, v + 0.15, height + 4.0),
            (x, v + d - 0.15, height + 4.0),
            [70, 83, 85],
        );
        let bolt = c.point((x, v + d, height + 0.5));
        c.dot(bolt.0 as i32, bolt.1 as i32, [186, 200, 192]);
    }
    for i in 0..(w * 3.0) as i32 {
        let x = u + 0.12 + f64::from(i) * 0.30;
        c.face(
            &[
                (x, v + d, 8.0),
                (x + 0.17, v + d, 8.0),
                (x + 0.17, v + d, 12.0),
                (x, v + d, 12.0),
            ],
            LAMP,
        );
    }
}

pub fn sprite_pixels(sprite: &BuildingSprite) -> (u32, u32, Vec<u8>) {
    let (width, height) = image_size(sprite.width, sprite.height);
    let (w, d) = (f64::from(sprite.width), f64::from(sprite.height));
    let mut c = Canvas::new(width, height, (d * 32.0, 32.0));
    // The concrete apron reaches the footprint corners; its north antenna is the fixed roof
    // height marker. Nothing moves the sprite's established ground anchor.
    c.block((0.0, 0.0, 0.0), (w, d, 2.0), [99, 103, 95]);
    for i in 1..sprite.width {
        c.beam(
            (f64::from(i), 0.0, 2.0),
            (f64::from(i), d, 2.0),
            [77, 82, 77],
        );
    }
    for i in 1..sprite.height {
        c.beam(
            (0.0, f64::from(i), 2.0),
            (w, f64::from(i), 2.0),
            [77, 82, 77],
        );
    }
    c.beam((0.0, 0.0, 2.0), (0.0, 0.0, 32.0), STEEL);
    c.dot((d * 32.0) as i32, 0, LAMP);
    match sprite.name {
        "construction_yard" => {
            hall(&mut c, (0.15, 0.22), (1.60, 1.15), 21.0);
            c.block((0.4, 0.5, 25.0), (0.7, 0.36, 4.0), [94, 105, 101]);
            // Crane legs and braced bridge over an open vehicle service apron.
            for u in [0.3, 2.65] {
                c.block((u, 1.65, 2.0), (0.16, 0.18, 40.0), TRIM);
                c.beam((u, 1.65, 4.0), (u + 0.18, 1.83, 40.0), DARK);
            }
            c.block((0.25, 1.60, 39.0), (2.6, 0.25, 6.0), [198, 155, 66]);
            for i in 0..12 {
                let u = 0.3 + f64::from(i) * 0.21;
                c.beam((u, 1.85, 40.0), (u + 0.18, 1.85, 45.0), DARK);
            }
            c.beam((2.0, 1.74, 39.0), (2.0, 1.74, 12.0), DARK);
            c.beam((2.0, 1.74, 12.0), (2.15, 1.74, 12.0), TRIM);
            c.block((0.3, 2.32, 2.0), (0.6, 0.36, 7.0), [137, 103, 61]);
            c.block((1.1, 2.45, 2.0), (0.4, 0.28, 4.0), STEEL);
        }
        "power_plant" => {
            hall(&mut c, (0.2, 0.2), (1.20, 0.65), 18.0);
            for u in [0.43, 1.05] {
                c.cylinder((u, 0.52, 22.0), 0.16, 17.0, [143, 147, 130]);
                c.cylinder((u, 0.52, 36.0), 0.17, 4.0, TEAM);
                c.ellipse(c.point((u, 0.52, 40.0)), (4.0, 2.0), DARK);
            }
            for u in [0.48, 1.30] {
                c.cylinder((u, 1.35, 3.0), 0.27, 10.0, [83, 114, 105]);
                let centre = c.point((u, 1.35, 13.0));
                c.ellipse(centre, (8.0, 4.0), DARK);
                c.line(
                    (centre.0 - 6.0, centre.1),
                    (centre.0 + 6.0, centre.1),
                    STEEL,
                );
                c.line(
                    (centre.0, centre.1 - 3.0),
                    (centre.0, centre.1 + 3.0),
                    STEEL,
                );
            }
        }
        "refinery" => {
            hall(&mut c, (0.16, 0.2), (1.15, 0.75), 23.0);
            c.cylinder((2.1, 0.8, 3.0), 0.47, 25.0, [167, 172, 157]);
            c.cylinder((2.1, 0.8, 17.0), 0.48, 5.0, TEAM);
            c.cylinder((0.7, 1.65, 3.0), 0.35, 16.0, [167, 172, 157]);
            for v in [1.45, 1.7] {
                c.block((1.25, v, 4.0), (1.18, 0.09, 3.0), [169, 137, 70]);
            }
            // Loading hopper and black/yellow dock chevrons.
            c.block((1.45, 2.05, 2.0), (1.0, 0.65, 7.0), DARK);
            c.block((1.55, 2.15, 9.0), (0.8, 0.4, 4.0), [166, 129, 51]);
            for i in 0..6 {
                let u = 1.46 + f64::from(i) * 0.16;
                c.beam((u, 2.76, 2.0), (u + 0.08, 2.56, 2.0), TRIM);
            }
        }
        "barracks" => {
            hall(&mut c, (0.2, 0.3), (1.1, 1.25), 16.0);
            c.block((0.4, 0.55, 20.0), (0.72, 0.8, 3.0), [143, 143, 115]);
            c.block((1.35, 0.30, 2.0), (0.4, 0.4, 28.0), [126, 129, 113]);
            c.block((1.30, 0.25, 27.0), (0.5, 0.5, 4.0), TEAM);
            c.face(
                &[
                    (0.63, 1.56, 3.0),
                    (0.99, 1.56, 3.0),
                    (0.99, 1.56, 13.0),
                    (0.63, 1.56, 13.0),
                ],
                DARK,
            );
            c.block((0.52, 1.57, 2.0), (0.59, 0.24, 2.0), TRIM);
            c.beam((1.7, 1.65, 2.0), (1.7, 1.65, 28.0), STEEL);
            c.face(
                &[
                    (1.7, 1.65, 28.0),
                    (1.95, 1.65, 26.0),
                    (1.95, 1.65, 21.0),
                    (1.7, 1.65, 23.0),
                ],
                TEAM,
            );
        }
        "war_factory" => {
            hall(&mut c, (0.2, 0.2), (2.25, 1.80), 27.0);
            for v in [0.4, 0.95, 1.5] {
                c.block((0.4, v, 31.0), (1.6, 0.24, 4.0), [145, 152, 148]);
                c.face(
                    &[
                        (0.5, v + 0.25, 31.0),
                        (1.9, v + 0.25, 31.0),
                        (1.9, v + 0.25, 34.0),
                        (0.5, v + 0.25, 34.0),
                    ],
                    LAMP,
                );
            }
            c.face(
                &[
                    (0.70, 2.01, 2.0),
                    (2.05, 2.01, 2.0),
                    (2.05, 2.01, 24.0),
                    (0.70, 2.01, 24.0),
                ],
                DARK,
            );
            c.block((0.65, 2.02, 2.0), (1.48, 0.60, 1.0), [72, 77, 73]);
            for u in [0.70, 2.05] {
                c.beam((u, 2.10, 3.0), (u, 2.8, 3.0), TRIM);
                c.beam((u, 2.02, 3.0), (u, 2.02, 23.0), [204, 157, 65]);
            }
            for i in 0..4 {
                let u = 0.83 + f64::from(i) * 0.29;
                c.beam((u, 2.02, 22.0), (u + 0.12, 2.02, 25.0), TRIM);
            }
        }
        "tech_center" => {
            hall(&mut c, (0.25, 0.2), (1.4, 1.3), 20.0);
            c.block((0.7, 0.7, 24.0), (0.4, 0.4, 7.0), DARK);
            let centre = c.point((0.9, 0.9, 39.0));
            c.ellipse(centre, (20.0, 9.0), STEEL);
            c.ellipse((centre.0, centre.1 - 2.0), (17.0, 6.0), [180, 197, 192]);
            c.line(
                (centre.0 - 16.0, centre.1),
                (centre.0 + 14.0, centre.1 - 2.0),
                DARK,
            );
            c.line((centre.0, centre.1 + 5.0), (centre.0, centre.1 - 7.0), DARK);
            c.beam((0.9, 0.9, 39.0), (0.64, 0.75, 49.0), TRIM);
            c.block((0.5, 1.55, 2.0), (0.8, 0.23, 9.0), TEAM);
        }
        "gun_turret" => {
            c.block((0.15, 0.15, 2.0), (0.70, 0.70, 8.0), [113, 121, 105]);
            c.cylinder((0.5, 0.5, 10.0), 0.23, 9.0, STEEL);
            c.block((0.3, 0.34, 19.0), (0.40, 0.35, 5.0), TEAM);
            c.block((0.58, 0.39, 20.0), (0.64, 0.10, 4.0), DARK);
        }
        "rocket_tower" => {
            c.block((0.15, 0.15, 2.0), (0.7, 0.7, 9.0), [113, 121, 105]);
            c.block((0.36, 0.36, 11.0), (0.28, 0.28, 13.0), STEEL);
            c.block((0.15, 0.22, 23.0), (0.70, 0.48, 8.0), TEAM);
            for i in 0..3 {
                let u = 0.22 + f64::from(i) * 0.2;
                c.block((u, 0.32, 29.0), (0.10, 0.54, 4.0), TRIM);
            }
        }
        _ => {}
    }
    // Outline the exposed front of the apron, keeping exact historical corner coverage.
    c.beam((0.0, d, 1.0), (w, d, 1.0), [148, 151, 129]);
    c.beam((w, 0.0, 1.0), (w, d, 1.0), [98, 104, 93]);
    c.dot(0, (d * 16.0 + 31.0) as i32, [148, 151, 129]);
    c.dot(width as i32 - 1, (w * 16.0 + 31.0) as i32, [98, 104, 93]);
    c.dot((w * 32.0) as i32, height as i32 - 1, [98, 104, 93]);
    (width, height, c.pixels)
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
