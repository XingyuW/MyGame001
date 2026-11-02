//! Ambient life (docs/DECISIONS.md, "Maps with relief"): townsfolk on the plazas, traffic on the
//! roads, herds in the pastures, boats at sea and gulls over the shore. Presentation only: the
//! simulation knows nothing of them, they never block or reveal anything, and they are drawn
//! only on cells the human player sees now. Where they appear follows from the map alone (an
//! integer hash of each cell), so every match on a map starts with the same life; how they
//! wander is the renderer's own, frame by frame.
//!
//! Their sheets are optional: a kind whose sheet (`sprites/hd/ambient/walk/<id>.png`, a walk
//! cycle as `sprites/hd/walk`, or `sprites/hd/ambient/units/<id>.png`, eight facings as
//! `sprites/hd/units`) is not installed does not appear.

use bevy::asset::{RenderAssetUsages, io::file::FileAssetReader};
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;
use sim::{CellPos, Direction};

use crate::battlefield::looks::{Feature, Ground, decode};
use crate::map::{MapLooks, MapTerrain, MapView};
use crate::sim_host::{CommandSink, SimHost};

pub struct AmbientPlugin;

impl Plugin for AmbientPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load)
            .add_systems(Update, (populate, wander, place).chain());
    }
}

/// Where a kind lives and moves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Habitat {
    /// Plazas and sidewalks of towns.
    Town,
    /// Roads, streets and bridges.
    Road,
    /// Open water at least a cell from any shore.
    Sea,
    /// Open grass and earth.
    Pasture,
    /// Grain fields.
    Fields,
    /// Open sand.
    Desert,
    /// Snow and ice.
    Snow,
    /// The air over a shore.
    Air,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Sheet {
    /// A walk cycle: eight facings of eight frames of 64 px.
    Walk,
    /// Eight facings of one 96 px frame.
    Vehicle,
}

struct Species {
    id: &'static str,
    sheet: Sheet,
    habitat: Habitat,
    /// Cells a second.
    speed: f32,
    /// One group in so many cells of its habitat, of up to `herd` members.
    rarity: u32,
    herd: u32,
    /// Seconds it may stand still between moves (grazing, waiting).
    rest: f32,
}

const SPECIES: [Species; 20] = [
    Species {
        id: "civ_man",
        sheet: Sheet::Walk,
        habitat: Habitat::Town,
        speed: 0.55,
        rarity: 60,
        herd: 1,
        rest: 3.0,
    },
    Species {
        id: "civ_woman",
        sheet: Sheet::Walk,
        habitat: Habitat::Town,
        speed: 0.5,
        rarity: 60,
        herd: 2,
        rest: 3.0,
    },
    Species {
        id: "civ_worker",
        sheet: Sheet::Walk,
        habitat: Habitat::Town,
        speed: 0.5,
        rarity: 140,
        herd: 1,
        rest: 5.0,
    },
    Species {
        id: "car_sedan",
        sheet: Sheet::Vehicle,
        habitat: Habitat::Road,
        speed: 1.8,
        rarity: 110,
        herd: 1,
        rest: 0.0,
    },
    Species {
        id: "car_compact",
        sheet: Sheet::Vehicle,
        habitat: Habitat::Road,
        speed: 1.8,
        rarity: 120,
        herd: 1,
        rest: 0.0,
    },
    Species {
        id: "taxi",
        sheet: Sheet::Vehicle,
        habitat: Habitat::Road,
        speed: 2.0,
        rarity: 200,
        herd: 1,
        rest: 0.0,
    },
    Species {
        id: "pickup",
        sheet: Sheet::Vehicle,
        habitat: Habitat::Road,
        speed: 1.6,
        rarity: 180,
        herd: 1,
        rest: 0.0,
    },
    Species {
        id: "bus",
        sheet: Sheet::Vehicle,
        habitat: Habitat::Road,
        speed: 1.2,
        rarity: 400,
        herd: 1,
        rest: 2.0,
    },
    Species {
        id: "truck",
        sheet: Sheet::Vehicle,
        habitat: Habitat::Road,
        speed: 1.3,
        rarity: 260,
        herd: 1,
        rest: 0.0,
    },
    Species {
        id: "tractor",
        sheet: Sheet::Vehicle,
        habitat: Habitat::Fields,
        speed: 0.5,
        rarity: 700,
        herd: 1,
        rest: 4.0,
    },
    Species {
        id: "cow",
        sheet: Sheet::Walk,
        habitat: Habitat::Pasture,
        speed: 0.2,
        rarity: 900,
        herd: 5,
        rest: 6.0,
    },
    Species {
        id: "sheep",
        sheet: Sheet::Walk,
        habitat: Habitat::Pasture,
        speed: 0.22,
        rarity: 1000,
        herd: 6,
        rest: 5.0,
    },
    Species {
        id: "horse",
        sheet: Sheet::Walk,
        habitat: Habitat::Fields,
        speed: 0.35,
        rarity: 900,
        herd: 3,
        rest: 5.0,
    },
    Species {
        id: "deer",
        sheet: Sheet::Walk,
        habitat: Habitat::Pasture,
        speed: 0.5,
        rarity: 1300,
        herd: 3,
        rest: 4.0,
    },
    Species {
        id: "camel",
        sheet: Sheet::Walk,
        habitat: Habitat::Desert,
        speed: 0.3,
        rarity: 900,
        herd: 3,
        rest: 5.0,
    },
    Species {
        id: "penguin",
        sheet: Sheet::Walk,
        habitat: Habitat::Snow,
        speed: 0.2,
        rarity: 800,
        herd: 6,
        rest: 4.0,
    },
    Species {
        id: "sailboat",
        sheet: Sheet::Vehicle,
        habitat: Habitat::Sea,
        speed: 0.6,
        rarity: 700,
        herd: 1,
        rest: 1.0,
    },
    Species {
        id: "fishing_boat",
        sheet: Sheet::Vehicle,
        habitat: Habitat::Sea,
        speed: 0.5,
        rarity: 900,
        herd: 1,
        rest: 3.0,
    },
    Species {
        id: "ferry",
        sheet: Sheet::Vehicle,
        habitat: Habitat::Sea,
        speed: 0.7,
        rarity: 2500,
        herd: 1,
        rest: 1.0,
    },
    Species {
        id: "seagull",
        sheet: Sheet::Walk,
        habitat: Habitat::Air,
        speed: 1.4,
        rarity: 500,
        herd: 2,
        rest: 0.0,
    },
];

/// The files of ambient life under `sprites` that the game may load: every species' sheet and
/// the manifest that says how they are made.
#[cfg(test)]
pub fn sprite_files() -> Vec<std::path::PathBuf> {
    SPECIES
        .iter()
        .map(|species| {
            let dir = match species.sheet {
                Sheet::Walk => "walk",
                Sheet::Vehicle => "units",
            };
            std::path::PathBuf::from(format!("hd/ambient/{dir}/{}.png", species.id))
        })
        .chain([std::path::PathBuf::from("hd/ambient/MANIFEST.md")])
        .collect()
}

/// The most ambient figures a map gets.
const MAX_LIFE: usize = 700;
/// How high a gull flies, in pixels.
const GULL_ALTITUDE: f32 = 52.0;
/// How far a gull strays from where it appeared, in cells.
const GULL_RANGE: f32 = 7.0;

/// The installed sheets, by species.
#[derive(Resource)]
struct AmbientArt(Vec<Option<Handle<Image>>>);

fn load(world: &mut World) {
    let root = FileAssetReader::new(AssetPlugin::default().file_path)
        .root_path()
        .clone();
    let sheets = SPECIES
        .iter()
        .map(|species| {
            let (dir, size) = match species.sheet {
                Sheet::Walk => ("walk", (2048, 2048)),
                Sheet::Vehicle => ("units", (3072, 384)),
            };
            let path = root
                .join("sprites/hd/ambient")
                .join(dir)
                .join(format!("{}.png", species.id));
            if !path.exists() {
                return None;
            }
            let bytes = match crate::tileset_image::check_png_file(&path, size.0, size.1) {
                Ok(bytes) => bytes,
                Err(error) => {
                    warn!("ambient sheet {}: {error}", path.display());
                    return None;
                }
            };
            let image = Image::from_buffer(
                &bytes,
                ImageType::Extension("png"),
                CompressedImageFormats::NONE,
                true,
                ImageSampler::linear(),
                RenderAssetUsages::default(),
            )
            .ok()?;
            Some(world.resource_mut::<Assets<Image>>().add(image))
        })
        .collect();
    world.insert_resource(AmbientArt(sheets));
}

/// One figure of ambient life.
#[derive(Component)]
struct Critter {
    species: usize,
    /// Continuous cell coordinates of its ground point.
    at: Vec2,
    target: Vec2,
    /// Where it appeared: a gull circles it.
    home: Vec2,
    facing: Direction,
    /// Cells walked, for the walk cycle.
    stride: f32,
    /// Seconds left standing still.
    wait: f32,
    seed: u32,
}

fn hash(x: i32, y: i32, seed: u32) -> u32 {
    let mut n = (x as u32).wrapping_mul(0x85eb_ca6b)
        ^ (y as u32).wrapping_mul(0xc2b2_ae35)
        ^ seed.wrapping_mul(0x27d4_eb2f);
    n ^= n >> 16;
    n = n.wrapping_mul(0x7feb_352d);
    n ^= n >> 15;
    n = n.wrapping_mul(0x846c_a68b);
    n ^ (n >> 16)
}

/// The habitats a cell of the map file offers (a bit per [`Habitat`]).
fn habitats(terrain: &sim::Grid, looks: &[u8], width: u32, cell: CellPos) -> u16 {
    let bit = |h: Habitat| 1_u16 << (h as u16);
    let Some(t) = terrain.cell(cell) else {
        return 0;
    };
    let look = looks
        .get((cell.y as u32 * width + cell.x as u32) as usize)
        .copied()
        .unwrap_or(0);
    let (ground, feature) = decode(look).unwrap_or((Ground::Auto, Feature::None));
    let water = |c: CellPos| terrain.cell(c).is_some_and(|c| c.water && !c.passable);
    let mut bits = 0;
    if water(cell) {
        let open =
            (-1..=1).all(|dy| (-1..=1).all(|dx| water(CellPos::new(cell.x + dx, cell.y + dy))));
        if open {
            bits |= bit(Habitat::Sea);
        }
    }
    let shore = (-2..=2).any(|dy| (-2..=2).any(|dx| water(CellPos::new(cell.x + dx, cell.y + dy))));
    if shore {
        bits |= bit(Habitat::Air);
    }
    if !t.passable {
        return bits;
    }
    match (ground, feature) {
        (_, Feature::Road | Feature::Bridge) => bits |= bit(Habitat::Road),
        (Ground::Pavement, Feature::None) => bits |= bit(Habitat::Town),
        (Ground::Grass | Ground::Dirt, Feature::None) => bits |= bit(Habitat::Pasture),
        (Ground::Crops, Feature::None) => bits |= bit(Habitat::Fields) | bit(Habitat::Pasture),
        (Ground::Sand, Feature::None) => bits |= bit(Habitat::Desert),
        (Ground::Snow | Ground::Ice, Feature::None) => bits |= bit(Habitat::Snow),
        _ => {}
    }
    bits
}

/// The habitats of every cell of the current map, row-major.
#[derive(Resource, Default)]
struct Habitats {
    width: u32,
    height: u32,
    bits: Vec<u16>,
}

impl Habitats {
    fn has(&self, cell: CellPos, habitat: Habitat) -> bool {
        (0..self.width as i32).contains(&cell.x)
            && (0..self.height as i32).contains(&cell.y)
            && self.bits[(cell.y as u32 * self.width + cell.x as u32) as usize]
                & (1 << habitat as u16)
                != 0
    }
}

/// Fills a newly started match's map with life.
fn populate(
    mut commands: Commands,
    terrain: Option<Res<MapTerrain>>,
    looks: Option<Res<MapLooks>>,
    map: Option<Res<MapView>>,
    art: Option<Res<AmbientArt>>,
    critters: Query<Entity, With<Critter>>,
) {
    let (Some(terrain), Some(map), Some(art)) = (terrain, map, art) else {
        return;
    };
    if !terrain.is_added() {
        return;
    }
    for entity in &critters {
        commands.entity(entity).despawn();
    }
    let grid = &terrain.0;
    let empty = Vec::new();
    let looks = looks.as_ref().map_or(&empty, |looks| &looks.0);
    let (width, height) = (map.width, map.height);
    let bits: Vec<u16> = (0..height as i32)
        .flat_map(|y| (0..width as i32).map(move |x| CellPos::new(x, y)))
        .map(|cell| habitats(grid, looks, width, cell))
        .collect();
    let habitats = Habitats {
        width,
        height,
        bits,
    };
    let mut count = 0;
    'cells: for y in 0..height as i32 {
        for x in 0..width as i32 {
            let cell = CellPos::new(x, y);
            for (index, species) in SPECIES.iter().enumerate() {
                if art.0[index].is_none()
                    || !habitats.has(cell, species.habitat)
                    || !hash(x, y, index as u32 + 0xa11).is_multiple_of(species.rarity)
                {
                    continue;
                }
                let members = 1 + hash(x, y, 0xbeef ^ index as u32) % species.herd;
                for member in 0..members {
                    let seed = hash(x, y, member * 977 + index as u32);
                    let offset = Vec2::new(
                        (seed % 100) as f32 / 100.0 - 0.5,
                        ((seed >> 8) % 100) as f32 / 100.0 - 0.5,
                    ) * if member == 0 { 0.4 } else { 1.6 };
                    let at = Vec2::new(x as f32 + 0.5, y as f32 + 0.5) + offset;
                    let spot = CellPos::new(at.x.floor() as i32, at.y.floor() as i32);
                    if !habitats.has(spot, species.habitat) {
                        continue;
                    }
                    commands.spawn((
                        Critter {
                            species: index,
                            at,
                            target: at,
                            home: at,
                            facing: Direction::ALL[(seed >> 16) as usize % 8],
                            stride: 0.0,
                            wait: (seed % 50) as f32 / 10.0,
                            seed,
                        },
                        Sprite {
                            image: art.0[index].clone().unwrap_or_default(),
                            custom_size: Some(Vec2::splat(match species.sheet {
                                Sheet::Walk => crate::hd_art::WALK_SIZE,
                                Sheet::Vehicle => crate::hd_art::UNIT_SIZE,
                            })),
                            ..default()
                        },
                        bevy::sprite::Anchor(match species.sheet {
                            Sheet::Walk => crate::hd_art::walk_anchor(),
                            Sheet::Vehicle => Vec2::ZERO,
                        }),
                        Transform::default(),
                        Visibility::Hidden,
                    ));
                    count += 1;
                    if count >= MAX_LIFE {
                        break 'cells;
                    }
                }
            }
        }
    }
    commands.insert_resource(habitats);
}

fn next_random(seed: &mut u32) -> u32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    *seed
}

/// Moves every figure: towards its target, and on to a neighbouring cell of its habitat that
/// nothing of the match stands on. Vehicles keep straight on while the road does.
fn wander(
    sink: Res<CommandSink>,
    speed: Res<crate::game_speed::GameClock>,
    host: Res<SimHost>,
    habitats: Option<Res<Habitats>>,
    mut critters: Query<&mut Critter>,
) {
    let Some(habitats) = habitats else {
        return;
    };
    if sink.paused() {
        return;
    }
    // Game time: life hurries with the match's speed.
    let dt = speed.delta().max(0.0);
    let grid = host.grid();
    let free = |cell: CellPos| {
        grid.is_none_or(|grid| {
            grid.building(cell).is_none()
                && grid.occupant(cell).is_none()
                && grid.cell(cell).is_some_and(|c| c.passable || c.water)
        })
    };
    for mut critter in &mut critters {
        let species = &SPECIES[critter.species];
        if critter.wait > 0.0 {
            critter.wait -= dt;
            continue;
        }
        let to = critter.target - critter.at;
        let step = species.speed * dt;
        if to.length() > step {
            let direction = to.normalize();
            critter.at += direction * step;
            critter.stride += step;
            continue;
        }
        let target = critter.target;
        critter.at = target;
        // Choose the next cell.
        let here = CellPos::new(target.x.floor() as i32, target.y.floor() as i32);
        let mut seed = critter.seed;
        let roll = next_random(&mut seed);
        critter.seed = seed;
        if species.rest > 0.0 && roll.is_multiple_of(4) {
            critter.wait = species.rest * ((roll >> 8) % 100) as f32 / 100.0;
            continue;
        }
        let straight = critter.facing.delta();
        let mut choices: Vec<(i32, i32)> = Direction::ALL
            .iter()
            .map(|d| d.delta())
            .filter(|&(dx, dy)| {
                let next = CellPos::new(here.x + dx, here.y + dy);
                let ok = habitats.has(next, species.habitat);
                match species.habitat {
                    // A gull flies anywhere near its shore, and stays near its home.
                    Habitat::Air => {
                        let point = Vec2::new(next.x as f32 + 0.5, next.y as f32 + 0.5);
                        point.distance(critter.home) <= GULL_RANGE
                    }
                    Habitat::Sea => ok,
                    _ => ok && free(next),
                }
            })
            .collect();
        if species.sheet == Sheet::Vehicle && species.habitat == Habitat::Road {
            // Traffic: straight on if it can, else any way but back.
            if choices.contains(&straight) {
                choices = vec![straight];
            } else if choices.len() > 1 {
                choices.retain(|&d| d != (-straight.0, -straight.1));
            }
        }
        let Some(&(dx, dy)) = choices.get((roll >> 4) as usize % choices.len().max(1)) else {
            critter.wait = 1.0;
            continue;
        };
        if let Some(&direction) = Direction::ALL.iter().find(|d| d.delta() == (dx, dy)) {
            critter.facing = direction;
        }
        let jitter = if species.sheet == Sheet::Walk {
            Vec2::new(
                ((roll >> 12) % 60) as f32 / 100.0 - 0.3,
                ((roll >> 20) % 60) as f32 / 100.0 - 0.3,
            )
        } else {
            Vec2::ZERO
        };
        critter.target = Vec2::new(
            here.x as f32 + dx as f32 + 0.5,
            here.y as f32 + dy as f32 + 0.5,
        ) + jitter;
    }
}

/// Draws every figure the human player sees, on its raised ground.
#[allow(clippy::type_complexity)]
fn place(
    host: Res<SimHost>,
    map: Option<Res<MapView>>,
    terrain: Option<Res<MapTerrain>>,
    reveal: Option<Res<crate::visual_review::RevealTerrain>>,
    mut critters: Query<(&Critter, &mut Sprite, &mut Transform, &mut Visibility)>,
) {
    let Some(map) = map else {
        return;
    };
    let vision = crate::fog::human_vision(host.players());
    for (critter, mut sprite, mut transform, mut visibility) in &mut critters {
        let species = &SPECIES[critter.species];
        let cell = CellPos::new(critter.at.x.floor() as i32, critter.at.y.floor() as i32);
        let seen = reveal.is_some() || vision.is_some_and(|vision| vision.is_visible(cell));
        let shown = if seen {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != shown {
            *visibility = shown;
        }
        if !seen {
            continue;
        }
        let flying = species.habitat == Habitat::Air;
        let mut ground =
            crate::relief::ground(terrain.as_deref(), critter.at.x, critter.at.y, map.tile);
        if flying {
            ground.y += GULL_ALTITUDE + (critter.stride * 3.0).sin() * 4.0;
        }
        let rect = match species.sheet {
            Sheet::Walk => {
                let moving = critter.wait <= 0.0 || flying;
                let phase = if flying {
                    critter.stride * 2.5 + critter.seed as f32 * 0.01
                } else {
                    critter.stride / crate::hd_art::WALK_CYCLE_CELLS * 1.6
                };
                let frame = if moving {
                    crate::hd_art::walk_frame(phase)
                } else {
                    0
                };
                crate::hd_art::walk_rect(critter.facing, frame)
            }
            Sheet::Vehicle => crate::hd_art::facing_rect(critter.facing),
        };
        if sprite.rect != Some(rect) {
            sprite.rect = Some(rect);
        }
        let depth = crate::units::unit_depth(critter.at) + if flying { 1.0 } else { 0.0 };
        transform.translation = Vec3::new(
            ground.x,
            ground.y + crate::hd_art::UNIT_GROUND_OFFSET,
            depth,
        );
    }
}
