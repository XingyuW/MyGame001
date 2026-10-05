//! Generates the unit sprite sheets `assets/sprites/units/<sprite>.png` (M2; the full roster of
//! both factions in M7).
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
//! wedge for the scout, and a broad box for the harvester (M3). M7 adds the full roster (docs/
//! DECISIONS.md D50): infantry drawn as small standing figures (stacked discs at increasing height:
//! legs, body, head) carrying a rifle, a rocket tube, or a grenade launcher pointing forward; the
//! raider as a wedge with a machine gun; the heavy tank with a broad hull and a large turret; the
//! artillery with a long gun; and the rocket launcher with a rack of tubes. The sprites use shaded steel, olive armour and blue team-paint accents; the game recolours only
//! those paint accents; the front of every sprite carries
//! the front shade (140) or the barrel shade (70), so that its facing can be checked.

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

/// A sprite: its name (as named by `sprite` in `assets/data/units.ron`) and shape.
pub struct SpriteKind {
    pub name: &'static str,
    /// Half length and half width of the hull, in cells (unused for infantry).
    hull: (f64, f64),
    turret: bool,
    /// Without a turret: a hull that narrows to a wedge at the front (scout) or a box (M3:
    /// harvester).
    wedge: bool,
    /// The M7 shapes; `None` for the three sprites of M2 and M3, which are drawn as before.
    extra: Option<Extra>,
}

/// The shapes added in M7 (docs/DECISIONS.md D50).
#[derive(Clone, Copy)]
enum Extra {
    /// A standing figure carrying a weapon of this length and half width (cells), at this height
    /// (pixels), with a pack on its back if `pack`.
    Infantry {
        weapon: f64,
        weapon_width: f64,
        weapon_lift: f64,
        pack: bool,
    },
    /// A turret of this radius with a barrel of this length and half width (cells), at this
    /// height (pixels).
    Gun {
        radius: f64,
        barrel: f64,
        barrel_width: f64,
        lift: f64,
    },
    /// A rack of rocket tubes over the rear of the hull.
    Rack,
}

const fn vehicle(name: &'static str, hull: (f64, f64), wedge: bool, extra: Extra) -> SpriteKind {
    SpriteKind {
        name,
        hull,
        turret: false,
        wedge,
        extra: Some(extra),
    }
}

const fn infantry(
    name: &'static str,
    weapon: f64,
    weapon_width: f64,
    weapon_lift: f64,
    pack: bool,
) -> SpriteKind {
    SpriteKind {
        name,
        hull: (0.0, 0.0),
        turret: false,
        wedge: false,
        extra: Some(Extra::Infantry {
            weapon,
            weapon_width,
            weapon_lift,
            pack,
        }),
    }
}

pub const SPRITES: [SpriteKind; 11] = [
    SpriteKind {
        name: "light_tank",
        hull: (0.34, 0.22),
        turret: true,
        wedge: false,
        extra: None,
    },
    SpriteKind {
        name: "scout",
        hull: (0.28, 0.14),
        turret: false,
        wedge: true,
        extra: None,
    },
    SpriteKind {
        name: "harvester",
        hull: (0.38, 0.26),
        turret: false,
        wedge: false,
        extra: None,
    },
    infantry("rifleman", 0.50, 0.03, 2.0, false),
    infantry("conscript", 0.46, 0.03, 2.0, true),
    infantry("rocket_soldier", 0.50, 0.05, 2.0, false),
    infantry("grenadier", 0.42, 0.045, 2.0, true),
    vehicle(
        "raider",
        (0.30, 0.16),
        true,
        Extra::Gun {
            radius: 0.07,
            barrel: 0.32,
            barrel_width: 0.03,
            lift: 5.0,
        },
    ),
    vehicle(
        "heavy_tank",
        (0.42, 0.28),
        false,
        Extra::Gun {
            radius: 0.17,
            barrel: 0.62,
            barrel_width: 0.06,
            lift: 7.0,
        },
    ),
    vehicle(
        "artillery",
        (0.30, 0.20),
        false,
        Extra::Gun {
            radius: 0.09,
            barrel: 0.78,
            barrel_width: 0.04,
            lift: 9.0,
        },
    ),
    vehicle("rocket_launcher", (0.36, 0.22), false, Extra::Rack),
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
    match kind.extra {
        Some(Extra::Infantry {
            weapon,
            weapon_width,
            weapon_lift,
            pack,
        }) => return infantry_pixel(&local, x, y, weapon, weapon_width, weapon_lift, pack),
        Some(Extra::Gun {
            radius,
            barrel,
            barrel_width,
            lift,
        }) => {
            let (along, across) = local(ground_point(x, y, lift));
            if (0.0..=barrel).contains(&along) && across.abs() <= barrel_width {
                return Some([70, 70, 70, 255]);
            }
            if along * along + across * across <= radius * radius {
                return Some(if along > radius * 0.4 {
                    [140, 140, 140, 255]
                } else if across > 0.02 {
                    [42, 133, 202, 255]
                } else if along < -radius * 0.2 && across < -radius * 0.1 {
                    [65, 80, 68, 255]
                } else {
                    [187, 194, 157, 255]
                });
            }
            for wall in 1..=4 {
                let (a, b) = local(ground_point(x, y, lift - f64::from(wall)));
                if a * a + b * b <= radius * radius {
                    return Some([76, 91, 74, 255]);
                }
            }
        }
        Some(Extra::Rack) => {
            let (along, across) = local(ground_point(x, y, 7.0));
            if (-0.30..=0.12).contains(&along) && across.abs() <= 0.17 {
                // Four tubes side by side; their open ends face forward.
                let tube = ((across + 0.17) / 0.085).floor();
                let between = ((across + 0.17) - tube * 0.085) < 0.015;
                return Some(if along > 0.06 {
                    [70, 70, 70, 255]
                } else if between {
                    [44, 54, 51, 255]
                } else {
                    [185, 180, 140, 255]
                });
            }
        }
        None => {}
    }
    if kind.turret {
        let (along, across) = local(ground_point(x, y, TURRET_LIFT));
        let barrel = (0.0..=0.5).contains(&along) && across.abs() <= 0.05;
        if barrel {
            return Some([70, 70, 70, 255]);
        }
        if along * along + across * across <= 0.15 * 0.15 {
            return Some(if across > 0.055 {
                [42, 133, 202, 255]
            } else if along < -0.04 && across < -0.02 {
                [56, 73, 60, 255]
            } else {
                [179, 189, 151, 255]
            });
        }
        for wall in 1..=3 {
            let (a, b) = local(ground_point(x, y, TURRET_LIFT - f64::from(wall)));
            if a * a + b * b <= 0.15 * 0.15 {
                return Some([70, 85, 67, 255]);
            }
        }
    }
    let (front_along, front_across) = local(ground_point(x, y, 0.0));
    if front_along > kind.hull.0 * 0.70
        && front_along <= kind.hull.0 + 0.05
        && front_across.abs() < kind.hull.1 * 0.50
    {
        return Some([140, 140, 140, 255]);
    }
    let (half_length, half_width) = kind.hull;
    // A raised armoured hull, individually shaded side skirts and segmented rubber tracks.
    // Sample top-down so the deck occludes the sides in every one of the eight facings.
    for lift in (0..=5).rev() {
        let (along, across) = local(ground_point(x, y, f64::from(lift)));
        let width_here = if kind.wedge {
            half_width * (1.0 - (along / half_length).max(0.0) * 0.55)
        } else {
            half_width
        };
        if along.abs() > half_length || across.abs() > width_here + 0.055 {
            continue;
        }
        let track = across.abs() > width_here - 0.045;
        if track {
            let tread = ((along + half_length) * 55.0) as i32 % 3;
            return Some(if lift > 3 {
                [87, 94, 81, 255]
            } else if tread == 0 {
                [81, 84, 75, 255]
            } else {
                [35, 42, 39, 255]
            });
        }
        if lift < 4 {
            return Some([66, 78, 69, 255]);
        }
        // Preserve the exact forward palette marker checked by the existing direction test.
        // The marker is on the ground-projected glacis, below the top deck.
        if along > half_length * 0.65 && lift == 5 {
            return Some([117, 133, 111, 255]);
        }
        if along < -half_length * 0.42 && !kind.wedge {
            if kind.name == "harvester" {
                return Some(
                    if ((along * 70.0) as i32 + (across * 70.0) as i32) % 5 == 0 {
                        [202, 161, 67, 255]
                    } else {
                        [137, 113, 61, 255]
                    },
                );
            }
            return Some(if (along * 75.0) as i32 % 3 == 0 {
                [45, 56, 52, 255]
            } else {
                [99, 113, 96, 255]
            });
        }
        if across.abs() > width_here * 0.48 && across.abs() < width_here * 0.78 {
            return Some([42, 133, 202, 255]);
        }
        if kind.wedge && along > -0.08 && along < 0.12 {
            return Some([66, 111, 125, 255]);
        }
        return Some(if across < 0.0 {
            [156, 166, 137, 255]
        } else {
            [126, 143, 116, 255]
        });
    }
    let (along, across) = local(ground_point(x, y, 0.0));
    if along > half_length * 0.60 && along <= half_length + 0.04 && across.abs() < half_width * 0.65
    {
        return Some([140, 140, 140, 255]);
    }
    let (u, v) = ground_point(x, y - y.min(2), 0.0);
    if (u * u + v * v) < (half_length + 0.10) * (half_length + 0.10) {
        return Some([20, 28, 22, 90]);
    }
    None
}

/// A standing figure: legs, body, and head as discs at increasing height, a weapon held low at its
/// right side and pointing forward at `weapon_lift` pixels (its front half in the barrel shade, which marks the
/// facing), and, if `pack`, a pack on its back. The highest part a pixel shows is drawn.
fn infantry_pixel(
    local: &dyn Fn((f64, f64)) -> (f64, f64),
    x: u32,
    y: u32,
    weapon: f64,
    weapon_width: f64,
    weapon_lift: f64,
    pack: bool,
) -> Option<[u8; 4]> {
    // From the top down, in steps of one pixel, so that nearer parts cover farther ones.
    let mut lift = 20.0;
    while lift >= 0.0 {
        let (along, across) = local(ground_point(x, y, lift));
        let disc = |radius: f64, centre: f64| {
            (along - centre) * (along - centre) + across * across <= radius * radius
        };
        // The weapon is held at the right side, so that it shows beside the body also when the
        // figure faces away from the viewer.
        if (lift - weapon_lift).abs() < 0.5
            && (0.0..=weapon).contains(&along)
            && (across - 0.10).abs() <= weapon_width
        {
            return Some(if along > weapon * 0.5 {
                [70, 70, 70, 255]
            } else {
                [110, 110, 110, 255]
            });
        }
        if (lift - 10.0).abs() < 0.5
            && (0.0..=weapon * 0.75).contains(&along)
            && (across - 0.11).abs() <= weapon_width
        {
            return Some([52, 62, 51, 255]);
        }
        if (9.0..14.0).contains(&lift) && along.abs() < 0.06 && (across.abs() - 0.12).abs() < 0.035
        {
            return Some(if lift < 11.0 {
                [180, 146, 104, 255]
            } else {
                [107, 119, 78, 255]
            });
        }
        if (16.0..=20.0).contains(&lift) && disc(0.07, 0.0) {
            return Some(if lift >= 18.0 {
                [130, 144, 107, 255]
            } else {
                [196, 157, 114, 255]
            });
        }
        if pack && (8.0..=14.0).contains(&lift) && disc(0.06, -0.09) {
            return Some([91, 89, 60, 255]);
        }
        if (6.0..16.0).contains(&lift) && disc(if lift > 12.0 { 0.10 } else { 0.08 }, 0.0) {
            return Some(if lift < 9.0 {
                [70, 84, 54, 255]
            } else if along > 0.025 {
                [42, 133, 202, 255]
            } else {
                [104, 121, 79, 255]
            });
        }
        if (0.0..6.0).contains(&lift) && along.abs() < 0.065 && (across.abs() - 0.055).abs() < 0.035
        {
            return Some([40, 49, 40, 255]);
        }
        lift -= 1.0;
    }
    None
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
