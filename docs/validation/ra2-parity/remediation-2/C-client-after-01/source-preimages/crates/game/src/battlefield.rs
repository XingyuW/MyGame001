//! Presentation-only terrain and time-based battlefield effects. Visibility is checked both at
//! emission and while particles live; effects never expose a hidden simulation entity.
use crate::{
    buildings::BuildingView,
    map::{MapTerrain, MapView},
    sim_host::{CommandSink, SimHost},
    units::UnitView,
};
use bevy::asset::{RenderAssetUsages, io::file::FileAssetReader};
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;
use sim::{CellPos, SimEvent, SimId};

#[path = "../examples/art/looks.rs"]
#[allow(dead_code)] // `look` builds values for the map generators.
pub(crate) mod looks;
use looks::{Feature, Ground as Soil};
pub struct BattlefieldPlugin;
#[derive(Resource)]
pub(crate) struct Materials {
    terrain: Vec<Handle<Image>>,
    pub(crate) glow: Handle<Image>,
    pub(crate) smoke: Handle<Image>,
    pub(crate) ring: Handle<Image>,
    crate_image: Handle<Image>,
}
#[derive(Component)]
pub(crate) struct BattlefieldVisual;
#[derive(Component)]
struct Ground {
    cell: CellPos,
    water: bool,
    phase: f32,
    /// Ore top layer: hidden once the player has seen the cell mined out.
    ore: bool,
    bridge: bool,
    depleted: bool,
    /// How far the sprite is drawn past its own diamond. It is shown no more revealed than any
    /// cell it covers, so it never paints over shroud or the void beyond the map edge.
    reach: Reach,
    /// Water: cells to the nearest shore (capped), which darkens open sea.
    depth: u8,
    /// Water: edges that meet a shore (bits as in [`NEIGHBOURS`]), where foam is drawn.
    foam: u8,
    /// The top of destructible scenery (`sim::Props`), removed when it is destroyed.
    scenery: bool,
    /// A colour the sprite is multiplied with, whatever the sight: basalt is the rock terrace,
    /// darkened.
    stain: [f32; 3],
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reach {
    /// Within its own diamond.
    Cell,
    /// Over the eight neighbouring cells (overlays with a ragged edge).
    Ring,
    /// Over the neighbours and the cells behind it on screen (city towers).
    Tall,
}
/// Terrain is drawn in three layers (see scripts/generate_hd_terrain.py). Bases and overlays
/// sample world-periodic textures, so the atlas slot follows the cell lattice.
const BASE_PERIOD: u32 = 8;
const SOIL_PERIOD: u32 = 4;
/// Every atlas is 1024 px square: within the bounded PNG loader's file and decode limits.
const TERRAIN_ATLAS: u32 = 1024;
// Bases (8 x 8 lattice).
const GRASS: usize = 0;
const WATER: usize = 2;
const PAVEMENT: usize = 10;
// Overlays (4 x 4 lattice, twice a cell's size, ragged edge).
const SOIL: usize = 4;
const SAND: usize = 8;
const SNOW: usize = 9;
// Tops: sixteen 64 px variants, or eight 64 x 128 px variants for the tall ones.
const ORE: usize = 1;
const CLIFF: usize = 3;
const ROAD: usize = 5;
const GROVE: usize = 6;
const COTTAGE: usize = 7;
const SNOW_CLIFF: usize = 11;
const MESA: usize = 12;
const SNOW_PINES: usize = 13;
const PALMS: usize = 14;
const BRIDGE: usize = 15;
const CITY: usize = 16;
const TOWER: usize = 17;
const SNOW_COTTAGE: usize = 18;
const ADOBE: usize = 19;
// More overlays (4 x 4 lattice, as soil): frozen water, grain fields, marsh, volcanic ash, lava.
const ICE: usize = 20;
const CROPS: usize = 21;
const MARSH: usize = 22;
const ASH: usize = 23;
const LAVA: usize = 24;
const GEMS: usize = 25;
pub(crate) const ATLASES: usize = 26;
/// Neighbours as (dx, dy, bit): 1 = -y, 2 = +x, 4 = +y, 8 = -x (the road and bridge variants).
const NEIGHBOURS: [(i32, i32, u8); 4] = [(0, -1, 1), (1, 0, 2), (0, 1, 4), (-1, 0, 8)];
/// What the rock terrace is multiplied with on volcanic ash: black basalt with a scorched cap.
const BASALT_STAIN: [f32; 3] = [0.64, 0.58, 0.60];
/// What the ore top is multiplied with on a cell of gems (RA5): the nuggets turn blue-green.
const GEM_STAIN: [f32; 3] = [0.42, 1.0, 0.96];
/// Cells more than this far from a shore draw as deep water.
const DEEP_WATER: u8 = 6;
#[derive(Component)]
struct Particle {
    cell: CellPos,
    age: f32,
    life: f32,
    velocity: Vec2,
    size: f32,
    growth: f32,
    color: [f32; 4],
}
#[derive(Resource, Default)]
struct AtmosphereClock {
    time: f32,
    last_puff: f32,
}

#[cfg(test)]
pub(crate) fn privacy_materials(app: &mut App) {
    app.insert_resource(Materials {
        terrain: vec![Handle::default(); ATLASES],
        glow: Handle::default(),
        smoke: Handle::default(),
        ring: Handle::default(),
        crate_image: Handle::default(),
    });
}
impl Plugin for BattlefieldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AtmosphereClock>()
            .add_systems(Startup, load.after(crate::skirmish::load_catalogue))
            .add_systems(
                Update,
                (
                    terrain,
                    raze_scenery,
                    bridge_states,
                    garrison_pips,
                    sprout_ore,
                    animate_ground,
                    show_crates,
                    effects,
                    animate_particles,
                )
                    .chain(),
            );
    }
}
fn load(world: &mut World) {
    let root = FileAssetReader::new(AssetPlugin::default().file_path)
        .root_path()
        .clone();
    let mut read = |name: &str, size: u32| -> Result<Handle<Image>, String> {
        let path = root.join("sprites/hd").join(format!("{name}.png"));
        let bytes =
            crate::tileset_image::check_png_file(&path, size, size).map_err(|e| e.to_string())?;
        let image = Image::from_buffer(
            &bytes,
            ImageType::Extension("png"),
            CompressedImageFormats::NONE,
            true,
            ImageSampler::linear(),
            RenderAssetUsages::default(),
        )
        .map_err(|e| e.to_string())?;
        Ok(world.resource_mut::<Assets<Image>>().add(image))
    };
    let result = (|| -> Result<Materials, String> {
        Ok(Materials {
            terrain: (0..ATLASES)
                .map(|i| read(&format!("terrain/{i}"), TERRAIN_ATLAS))
                .collect::<Result<_, _>>()?,
            glow: read("glow", 128)?,
            smoke: read("smoke", 128)?,
            ring: read("ring", 128)?,
            crate_image: read("crate", 128)?,
        })
    })();
    match result {
        Ok(materials) => {
            world.insert_resource(materials);
        }
        Err(e) => {
            error!("HD battlefield: {e}");
            world.write_message(AppExit::error());
        }
    }
}
/// The ground and feature a cell shows: its look, or for look 0 what its terrain suggests.
fn resolve(cell: &sim::Cell, look: u8) -> (Soil, Feature) {
    match looks::decode(look) {
        Some((ground, feature)) if ground != Soil::Auto => (ground, feature),
        _ if cell.water => (Soil::Water, Feature::None),
        _ if !cell.passable => (Soil::Grass, Feature::Cliff),
        _ if cell.ore > 0 => (Soil::Grass, Feature::Ore),
        _ if !cell.buildable => (Soil::Dirt, Feature::None),
        _ => (Soil::Grass, Feature::None),
    }
}
/// Cells to the nearest non-water cell, for every water cell (0 on land), capped.
fn water_depths(grid: &sim::Grid) -> Vec<u8> {
    let (w, h) = (grid.width() as i32, grid.height() as i32);
    let index = |x: i32, y: i32| (y * w + x) as usize;
    let mut depth = vec![u8::MAX; (w * h) as usize];
    let mut queue = std::collections::VecDeque::new();
    for y in 0..h {
        for x in 0..w {
            if grid.cell(CellPos::new(x, y)).is_some_and(|c| !c.water) {
                depth[index(x, y)] = 0;
                queue.push_back((x, y));
            }
        }
    }
    while let Some((x, y)) = queue.pop_front() {
        let next = depth[index(x, y)].saturating_add(1).min(DEEP_WATER);
        for (dx, dy, _) in NEIGHBOURS {
            let (nx, ny) = (x + dx, y + dy);
            if (0..w).contains(&nx) && (0..h).contains(&ny) && depth[index(nx, ny)] > next {
                depth[index(nx, ny)] = next;
                queue.push_back((nx, ny));
            }
        }
    }
    // A map of only water has no shore at all: all of it is open sea.
    depth.iter_mut().for_each(|d| *d = (*d).min(DEEP_WATER));
    depth
}
fn terrain(
    mut commands: Commands,
    map: Option<Res<MapView>>,
    terrain: Option<Res<MapTerrain>>,
    looks: Option<Res<crate::map::MapLooks>>,
    materials: Option<Res<Materials>>,
) {
    let (Some(map), Some(terrain), Some(materials)) = (map, terrain, materials) else {
        return;
    };
    if !terrain.is_added() {
        return;
    }
    let grid = &terrain.0;
    let look_of = |cell: CellPos| -> Option<(Soil, Feature)> {
        let t = grid.cell(cell)?;
        let index = (cell.y as u32 * map.width + cell.x as u32) as usize;
        let look = looks.as_ref().and_then(|l| l.0.get(index).copied());
        Some(resolve(t, look.unwrap_or(0)))
    };
    let depths = water_depths(grid);
    for y in 0..map.height {
        for x in 0..map.width {
            let cell = CellPos::new(x as i32, y as i32);
            let Some((ground, feature)) = look_of(cell) else {
                continue;
            };
            let neighbour = |dx: i32, dy: i32| look_of(CellPos::new(cell.x + dx, cell.y + dy));
            let is_water = |c: Option<(Soil, Feature)>| {
                c.is_some_and(|(g, f)| g == Soil::Water && f != Feature::Bridge)
            };
            let shore = ground != Soil::Water
                && NEIGHBOURS
                    .iter()
                    .any(|&(dx, dy, _)| is_water(neighbour(dx, dy)));
            let foam = if ground == Soil::Water && feature != Feature::Bridge {
                NEIGHBOURS.iter().fold(0, |bits, &(dx, dy, bit)| {
                    let next = neighbour(dx, dy);
                    if next.is_some()
                        && !is_water(next)
                        && next.map(|n| n.1) != Some(Feature::Bridge)
                    {
                        bits | bit
                    } else {
                        bits
                    }
                })
            } else {
                0
            };
            let position = crate::projection::cell_centre(cell, map.tile);
            let depth = (x + y) as f32 / 16384.0;
            let lattice = |period: u32| (x % period) + period * (y % period);
            let mut layer = |image: usize, slot: u32, size: Vec2, z: f32, reach: Reach| {
                // Square slots fill a row of `per_row`; tall ones are twice as high.
                let tall = size.y > size.x * 1.5;
                let per_row = match image {
                    GRASS | WATER | PAVEMENT => BASE_PERIOD,
                    SOIL | SAND | SNOW | ICE | CROPS | MARSH | ASH | LAVA => SOIL_PERIOD,
                    _ => 4,
                };
                let px = TERRAIN_ATLAS / per_row;
                let py = if tall { px * 2 } else { px };
                let (left, top) = ((slot % per_row) * px, (slot / per_row) * py);
                let lift = if tall { (size.y - size.x) / 2.0 } else { 0.0 };
                commands.spawn((
                    BattlefieldVisual,
                    Ground {
                        cell,
                        water: image == WATER,
                        phase: slot as f32,
                        ore: image == ORE || image == GEMS,
                        bridge: image == BRIDGE,
                        depleted: false,
                        reach,
                        depth: if image == WATER {
                            depths[(y * map.width + x) as usize]
                        } else {
                            0
                        },
                        foam: if image == WATER { foam } else { 0 },
                        scenery: matches!(
                            image,
                            GROVE
                                | COTTAGE
                                | SNOW_PINES
                                | PALMS
                                | CITY
                                | TOWER
                                | SNOW_COTTAGE
                                | ADOBE
                        ),
                        stain: if image == CLIFF && ground == Soil::Ash {
                            BASALT_STAIN
                        } else if image == GEMS {
                            GEM_STAIN
                        } else if image == LAVA {
                            // A flow glows unevenly, which also hides the texture's period.
                            let (fx, fy) = (x as f32, y as f32);
                            let glow =
                                (fx * 0.37 + fy * 0.21).sin() * (fx * 0.13 - fy * 0.29).cos();
                            [0.84 + 0.16 * glow, 0.80 + 0.20 * glow, 0.78 + 0.22 * glow]
                        } else {
                            [1.0; 3]
                        },
                    },
                    Sprite {
                        image: materials.terrain[image].clone(),
                        rect: Some(Rect::new(
                            left as f32,
                            top as f32,
                            (left + px) as f32,
                            (top + py) as f32,
                        )),
                        custom_size: Some(size),
                        color: Color::BLACK,
                        ..default()
                    },
                    Transform::from_translation((position + Vec2::Y * lift).extend(z)),
                ));
            };
            let cell_size = Vec2::splat(64.4);
            let overlay_size = Vec2::splat(128.8);
            // Base: continuous grass, water or pavement. Bridges span water.
            let base = match ground {
                Soil::Water => WATER,
                Soil::Pavement => PAVEMENT,
                _ => GRASS,
            };
            layer(
                base,
                lattice(BASE_PERIOD),
                cell_size,
                0.1 + depth,
                Reach::Cell,
            );
            // Overlays spill past their diamond with a ragged edge: soft transitions, beaches.
            let overlay = match ground {
                Soil::Dirt => Some(SOIL),
                Soil::Sand => Some(SAND),
                Soil::Snow => Some(SNOW),
                Soil::Ice => Some(ICE),
                Soil::Crops => Some(CROPS),
                Soil::Marsh => Some(MARSH),
                Soil::Ash => Some(ASH),
                Soil::Grass if matches!(feature, Feature::Cliff) => Some(SOIL),
                Soil::Grass if shore => Some(SAND),
                _ => None,
            };
            if let Some(overlay) = overlay {
                layer(
                    overlay,
                    lattice(SOIL_PERIOD),
                    overlay_size,
                    0.3 + depth,
                    Reach::Ring,
                );
            }
            // Mined ground under ore and gems, whatever the ground around it.
            if matches!(feature, Feature::Ore | Feature::Gems) {
                layer(
                    SOIL,
                    lattice(SOIL_PERIOD),
                    overlay_size,
                    0.32 + depth,
                    Reach::Ring,
                );
            }
            // A lava flow lies on its ground like an overlay, and spills over its banks.
            if feature == Feature::Lava {
                layer(
                    LAVA,
                    lattice(SOIL_PERIOD),
                    overlay_size,
                    0.34 + depth,
                    Reach::Ring,
                );
            }
            // Tops: rendered miniatures with baked shadows.
            let connected = || {
                NEIGHBOURS.iter().fold(0_u32, |bits, &(dx, dy, bit)| {
                    if neighbour(dx, dy)
                        .is_some_and(|(_, f)| matches!(f, Feature::Road | Feature::Bridge))
                    {
                        bits | u32::from(bit)
                    } else {
                        bits
                    }
                })
            };
            let scatter = (x.wrapping_mul(31) ^ y.wrapping_mul(71)) % 16;
            let top = match (feature, ground) {
                (
                    Feature::None
                    | Feature::Lava
                    | Feature::OilDerrick
                    | Feature::Hospital
                    | Feature::Airport
                    | Feature::TechOutpost
                    | Feature::BridgeHut,
                    _,
                ) => None,
                (Feature::Ore, _) => Some((ORE, scatter)),
                (Feature::Gems, _) => Some((GEMS, scatter)),
                (Feature::Cliff, Soil::Snow | Soil::Ice) => Some((SNOW_CLIFF, scatter)),
                (Feature::Cliff, Soil::Sand) => Some((MESA, scatter)),
                (Feature::Cliff, _) => Some((CLIFF, scatter)),
                (Feature::Road, _) => Some((ROAD, connected())),
                (Feature::Bridge, _) => Some((BRIDGE, connected())),
                (Feature::Forest, Soil::Snow | Soil::Ice) => Some((SNOW_PINES, scatter)),
                (Feature::Forest, Soil::Sand) => Some((PALMS, scatter)),
                (Feature::Forest, _) => Some((GROVE, scatter)),
                (Feature::House, Soil::Snow | Soil::Ice) => Some((SNOW_COTTAGE, scatter)),
                (Feature::House, Soil::Sand) => Some((ADOBE, scatter)),
                (Feature::House, _) => Some((COTTAGE, scatter)),
                (Feature::City, _) => Some((CITY, scatter % 8)),
                (Feature::Tower, _) => Some((TOWER, scatter % 8)),
            };
            if let Some((image, variant)) = top {
                if matches!(image, CITY | TOWER) {
                    // Tall blocks sort with units and buildings, so they hide what stands
                    // behind them.
                    let centre = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                    layer(
                        image,
                        variant,
                        Vec2::new(64.4, 128.8),
                        crate::units::unit_depth(centre),
                        Reach::Tall,
                    );
                } else {
                    layer(image, variant, cell_size, 0.5 + depth, Reach::Cell);
                }
            }
        }
    }
}
/// Clears observed destroyed scenery from the picture: its top goes, churned
/// earth is left, and the map the interface reads (`MapTerrain`, `MapLooks`: picking, order
/// preview, minimap) shows open ground there. The blast itself is drawn by [`effects`].
fn bridge_states(
    host: Res<SimHost>,
    mut terrain: Option<ResMut<MapTerrain>>,
    mut looks: Option<ResMut<crate::map::MapLooks>>,
    mut last: Local<Option<u64>>,
) {
    if *last == Some(host.tick()) {
        return;
    }
    *last = Some(host.tick());
    let (Some(grid), Some(bridges)) = (host.grid(), host.world().get_resource::<sim::Bridges>())
    else {
        return;
    };
    let vision = crate::fog::human_vision(host.players());
    for bridge in bridges.bridges.values() {
        for &cell in &bridge.cells {
            // Fog keeps the last observed terrain and deck state. Both navigation previews
            // and the minimap reconcile physical changes only when the cell is observed again.
            if vision.is_some_and(|v| v.is_visible(cell)) {
                if let (Some(terrain), Some(value)) = (terrain.as_mut(), grid.cell(cell)) {
                    terrain.0.set_terrain(cell, *value);
                }
                let index = cell.y as usize * grid.width() as usize + cell.x as usize;
                if let Some(look) = looks.as_mut().and_then(|l| l.0.get_mut(index)) {
                    *look = if bridge.collapsed { 6 } else { 6 | (8 << 4) };
                }
            }
        }
    }
}

fn garrison_pips(host: Res<SimHost>, map: Option<Res<MapView>>, mut gizmos: Gizmos) {
    let (Some(map), Some(state)) = (map, host.world().get_resource::<sim::SpecialState>()) else {
        return;
    };
    let Some(vision) = crate::fog::human_vision(host.players()) else {
        return;
    };
    for (id, g) in &state.garrisons {
        if g.owner != crate::units::HUMAN && !vision.is_visible(g.cell) {
            continue;
        }
        let count = state.cargo.get(id).map_or(0, Vec::len);
        let point = crate::projection::cell_centre(g.cell, map.tile)
            + Vec2::new(-(count as f32) * 3.0, 32.0);
        let color = if g.owner == crate::units::HUMAN {
            Color::srgb(0.3, 1.0, 0.3)
        } else {
            Color::srgb(1.0, 0.3, 0.2)
        };
        for i in 0..count {
            gizmos.rect_2d(
                point + Vec2::new(i as f32 * 6.0, 0.0),
                Vec2::new(4.0, 5.0),
                color,
            );
        }
    }
}

fn raze_scenery(
    mut commands: Commands,
    host: Res<SimHost>,
    map: Option<Res<MapView>>,
    materials: Option<Res<Materials>>,
    mut terrain: Option<ResMut<MapTerrain>>,
    mut looks: Option<ResMut<crate::map::MapLooks>>,
    ground: Query<(Entity, &Ground)>,
) {
    let razed: std::collections::BTreeSet<CellPos> = ground
        .iter()
        .filter(|(_, tile)| {
            tile.scenery
                && host
                    .observed_scenery(tile.cell)
                    .is_some_and(|observed| observed.prop.is_none())
        })
        .map(|(_, tile)| tile.cell)
        .collect();
    let (Some(map), Some(materials)) = (map, materials) else {
        return;
    };
    if razed.is_empty() {
        return;
    }
    for (entity, tile) in &ground {
        if tile.scenery && razed.contains(&tile.cell) {
            commands.entity(entity).despawn();
        }
    }
    for cell in razed {
        let (Ok(x), Ok(y)) = (u32::try_from(cell.x), u32::try_from(cell.y)) else {
            continue;
        };
        if let Some(terrain) = terrain.as_mut() {
            terrain.0.set_terrain(
                cell,
                sim::Cell {
                    passable: true,
                    water: false,
                    buildable: true,
                    ore: 0,
                },
            );
        }
        // The feature goes, the ground stays.
        if let Some(look) = looks
            .as_mut()
            .and_then(|looks| looks.0.get_mut((y * map.width + x) as usize))
        {
            *look &= 15;
        }
        let px = (TERRAIN_ATLAS / SOIL_PERIOD) as f32;
        let (left, top) = ((x % SOIL_PERIOD) as f32 * px, (y % SOIL_PERIOD) as f32 * px);
        commands.spawn((
            BattlefieldVisual,
            Ground {
                cell,
                water: false,
                phase: 0.0,
                ore: false,
                bridge: false,
                depleted: false,
                reach: Reach::Ring,
                depth: 0,
                foam: 0,
                scenery: false,
                stain: [1.0; 3],
            },
            Sprite {
                image: materials.terrain[SOIL].clone(),
                rect: Some(Rect::new(left, top, left + px, top + px)),
                custom_size: Some(Vec2::splat(128.8)),
                color: Color::BLACK,
                ..default()
            },
            Transform::from_translation(
                crate::projection::cell_centre(cell, map.tile)
                    .extend(0.34 + (x + y) as f32 / 16384.0),
            ),
        ));
    }
}
/// The cells a sprite of `reach` covers besides its own.
fn covered(reach: Reach) -> &'static [(i32, i32)] {
    const RING: [(i32, i32); 8] = [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ];
    const TALL: [(i32, i32); 14] = [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
        (-2, -2),
        (-2, -1),
        (-1, -2),
        (-3, -3),
        (-3, -2),
        (-2, -3),
    ];
    match reach {
        Reach::Cell => &[],
        Reach::Ring => &RING,
        Reach::Tall => &TALL,
    }
}
/// How often the simulation's grid is searched for ore that spread onto new ground, in ticks.
const ORE_SEARCH_TICKS: u64 = 20;
/// Ore that spread onto ground the map file has no ore on (RA5, `sim`'s `ore` module) gets an
/// ore top of its own, hidden until the player sees the cell, as a mined-out cell's top is
/// shown again when ore spreads back onto it ([`animate_ground`]).
fn sprout_ore(
    mut commands: Commands,
    host: Res<SimHost>,
    map: Option<Res<MapView>>,
    materials: Option<Res<Materials>>,
    ground: Query<&Ground>,
    mut searched: Local<Option<u64>>,
) {
    let (Some(map), Some(materials), Some(grid)) = (map, materials, host.grid()) else {
        return;
    };
    let tick = host.tick();
    if *searched == Some(tick) || !tick.is_multiple_of(ORE_SEARCH_TICKS) {
        return;
    }
    *searched = Some(tick);
    let topped: std::collections::HashSet<CellPos> = ground
        .iter()
        .filter(|tile| tile.ore)
        .map(|tile| tile.cell)
        .collect();
    for y in 0..map.height {
        for x in 0..map.width {
            let cell = CellPos::new(x as i32, y as i32);
            if !grid.cell(cell).is_some_and(|c| c.ore > 0) || topped.contains(&cell) {
                continue;
            }
            let scatter = (x.wrapping_mul(31) ^ y.wrapping_mul(71)) % 16;
            let px = TERRAIN_ATLAS / 4;
            let (left, top) = ((scatter % 4) * px, (scatter / 4) * px);
            let position = crate::projection::cell_centre(cell, map.tile);
            commands.spawn((
                BattlefieldVisual,
                Ground {
                    cell,
                    water: false,
                    phase: scatter as f32,
                    ore: true,
                    bridge: false,
                    // Shown once the player sees the cell.
                    depleted: true,
                    reach: Reach::Cell,
                    depth: 0,
                    foam: 0,
                    scenery: false,
                    stain: if grid.is_gems(cell) {
                        GEM_STAIN
                    } else {
                        [1.0; 3]
                    },
                },
                Sprite {
                    image: materials.terrain[if grid.is_gems(cell) { GEMS } else { ORE }].clone(),
                    rect: Some(Rect::new(
                        left as f32,
                        top as f32,
                        (left + px) as f32,
                        (top + px) as f32,
                    )),
                    custom_size: Some(Vec2::splat(64.4)),
                    color: Color::NONE,
                    ..default()
                },
                Transform::from_translation(position.extend(0.5 + (x + y) as f32 / 16384.0)),
            ));
        }
    }
}
/// A crate on the map (RA5).
#[derive(Component)]
struct CrateMark(CellPos);
/// The size of a crate's mark in pixels: well inside its cell.
const CRATE_SIZE: f32 = 22.0;
/// Draws the crates of the match that the human player sees now (RA5, `sim::Crates`): a plain
/// wooden box in the cell, gone when the crate is taken or the cell is out of sight. A
/// original generated timber-and-steel crate with a separate contact shadow.
fn show_crates(
    mut commands: Commands,
    host: Res<SimHost>,
    map: Option<Res<MapView>>,
    marks: Query<(Entity, &CrateMark)>,
    materials: Option<Res<Materials>>,
    reveal: Option<Res<crate::visual_review::RevealTerrain>>,
) {
    let seen: std::collections::BTreeSet<CellPos> = match (
        host.world().get_resource::<sim::Crates>(),
        crate::fog::human_vision(host.players()),
    ) {
        (Some(crates), Some(vision)) => crates
            .cells()
            .filter(|&cell| reveal.is_some() || vision.is_visible(cell))
            .collect(),
        _ => Default::default(),
    };
    let mut drawn = std::collections::BTreeSet::new();
    for (entity, mark) in &marks {
        if seen.contains(&mark.0) {
            drawn.insert(mark.0);
        } else {
            commands.entity(entity).despawn();
        }
    }
    let Some(map) = map else {
        return;
    };
    for cell in seen.difference(&drawn) {
        let position = crate::projection::cell_centre(*cell, map.tile);
        let depth = 0.6 + (cell.x + cell.y) as f32 / 16384.0;
        // A box: its side, and a lighter lid a little above it.
        for (colour, size, lift, z) in [
            (Color::srgba(0.1, 0.1, 0.08, 0.35), CRATE_SIZE, -4.0, depth),
            (Color::WHITE, CRATE_SIZE, 5.0, depth + 0.00001),
        ] {
            commands.spawn((
                BattlefieldVisual,
                CrateMark(*cell),
                Sprite {
                    image: if lift > 0.0 {
                        materials
                            .as_ref()
                            .map(|m| m.crate_image.clone())
                            .unwrap_or_default()
                    } else {
                        Handle::default()
                    },
                    color: colour,
                    custom_size: Some(Vec2::new(size, if lift > 0.0 { size } else { size * 0.4 })),
                    ..default()
                },
                Transform::from_translation((position + Vec2::Y * lift).extend(z)),
            ));
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn animate_ground(
    host: Res<SimHost>,
    clock: Res<AtmosphereClock>,
    mut ground: Query<(&mut Ground, &mut Sprite)>,
    map: Option<Res<MapView>>,
    mut gizmos: Gizmos,
    mut last_tick: Local<Option<u64>>,
    reveal: Option<Res<crate::visual_review::RevealTerrain>>,
) {
    let Some(vision) = crate::fog::human_vision(host.players()) else {
        return;
    };
    // Sight and ore change only with a simulation tick; water also shimmers every frame.
    let fresh = *last_tick != Some(host.tick());
    *last_tick = Some(host.tick());
    for (mut tile, mut sprite) in &mut ground {
        if !fresh && !tile.water && !tile.is_added() {
            continue;
        }
        let mut sight = crate::fog::sight_of(vision, tile.cell);
        for (dx, dy) in covered(tile.reach) {
            let next = CellPos::new(tile.cell.x + dx, tile.cell.y + dy);
            sight = sight.min(crate::fog::sight_of(vision, next));
        }
        if reveal.is_some() {
            sight = crate::fog::Sight::Visible;
        }
        // Depletion is only learned while the cell is visible; fog keeps the last seen state.
        let depleted = if tile.ore && sight == crate::fog::Sight::Visible {
            !host
                .grid()
                .and_then(|g| g.cell(tile.cell))
                .is_some_and(|c| c.ore > 0)
        } else if tile.bridge && sight == crate::fog::Sight::Visible {
            host.grid()
                .and_then(|g| g.cell(tile.cell))
                .is_some_and(|c| c.water)
        } else {
            tile.depleted
        };
        // Open water darkens and turns bluer away from the shore.
        let deep = f32::from(tile.depth.saturating_sub(1)) / f32::from(DEEP_WATER - 1);
        let sea = Vec3::new(1.0 - 0.42 * deep, 1.0 - 0.30 * deep, 1.0 - 0.12 * deep);
        let color = if tile.water && sight == crate::fog::Sight::Visible {
            let shimmer = 0.96 + 0.04 * (clock.time * 1.3 + tile.phase * 0.7).sin();
            Color::srgb(shimmer * sea.x, shimmer * sea.y, sea.z)
        } else {
            let shade = if tile.water {
                sea
            } else {
                Vec3::from(tile.stain)
                    * (0.94
                        + 0.06
                            * ((tile.cell.x as f32 * 0.23).sin()
                                * (tile.cell.y as f32 * 0.19).cos()))
            };
            let tint = crate::fog::tint(sight).to_srgba();
            Color::srgba(
                tint.red * shade.x,
                tint.green * shade.y,
                tint.blue * shade.z,
                if depleted { 0.0 } else { tint.alpha },
            )
        };
        if tile.foam != 0
            && sight == crate::fog::Sight::Visible
            && let Some(map) = &map
        {
            let corners = crate::projection::cell_corners(tile.cell, map.tile);
            for (bit, a, b) in [(1, 0, 1), (2, 1, 2), (4, 2, 3), (8, 3, 0)] {
                if tile.foam & bit == 0 {
                    continue;
                }
                for i in 0..8 {
                    let start = corners[a].lerp(corners[b], i as f32 / 8.0);
                    let end = corners[a].lerp(corners[b], (i + 1) as f32 / 8.0);
                    let drift = Vec2::Y * (clock.time * 1.4 + i as f32 * 0.8).sin() * 0.65;
                    gizmos.line_2d(
                        start + drift,
                        end + drift,
                        Color::srgba(0.57, 0.75, 0.70, 0.45),
                    );
                }
            }
        }
        if depleted != tile.depleted {
            tile.depleted = depleted;
        }
        if sprite.color != color {
            sprite.color = color;
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn puff(
    commands: &mut Commands,
    image: &Handle<Image>,
    cell: CellPos,
    pos: Vec2,
    velocity: Vec2,
    size: f32,
    growth: f32,
    life: f32,
    color: [f32; 4],
) {
    commands.spawn((
        BattlefieldVisual,
        Particle {
            cell,
            age: 0.0,
            life,
            velocity,
            size,
            growth,
            color,
        },
        Sprite {
            image: image.clone(),
            custom_size: Some(Vec2::splat(size)),
            color: Color::srgba(color[0], color[1], color[2], color[3]),
            ..default()
        },
        Transform::from_translation(pos.extend(3.5)),
    ));
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn effects(
    mut commands: Commands,
    time: Res<Time>,
    sink: Res<CommandSink>,
    host: Res<SimHost>,
    map: Option<Res<MapView>>,
    materials: Option<Res<Materials>>,
    mut clock: ResMut<AtmosphereClock>,
    mut events: MessageReader<SimEvent>,
    units: Query<(&UnitView, &Transform, &Visibility)>,
    buildings: Query<(&BuildingView, &Transform, &Visibility)>,
    particles: Query<&Particle>,
) {
    let (Some(map), Some(mat), Some(vision)) =
        (map, materials, crate::fog::human_vision(host.players()))
    else {
        events.clear();
        return;
    };
    if sink.paused() {
        events.clear();
        return;
    }
    clock.time += time.delta_secs();
    let mut budget = 256_usize.min(1600_usize.saturating_sub(particles.iter().count()));
    let points: std::collections::BTreeMap<SimId, (Vec2, CellPos)> = units
        .iter()
        .filter(|(_, _, v)| **v != Visibility::Hidden)
        .map(|(v, t, _)| {
            (
                v.id,
                (t.translation.truncate() - Vec2::Y * 12.0, v.current.cell()),
            )
        })
        .chain(
            buildings
                .iter()
                .filter(|(_, _, v)| **v != Visibility::Hidden)
                .filter_map(|(v, t, _)| {
                    let b = host.world().get::<sim::Building>(v.building);
                    // A destroyed building's view still carries its last location until PostUpdate.
                    let cell = b.map_or_else(
                        || {
                            crate::projection::pick_cell(
                                t.translation.truncate() - Vec2::Y * crate::hd_art::BUILDING_RISE,
                                map.tile,
                                map.width,
                                map.height,
                            )
                        },
                        |b| Some(b.footprint.origin),
                    )?;
                    Some((v.id, (t.translation.truncate(), cell)))
                }),
        )
        .collect();
    for event in events.read() {
        // Shots and what they fire are drawn by `crate::weapon_fx`.
        let (id, large, shot) = match event {
            SimEvent::UnitDestroyed { unit, .. } => (*unit, false, false),
            SimEvent::BuildingDestroyed { building, .. } => (*building, true, false),
            SimEvent::PropDestroyed { prop, kind, .. } => {
                (*prop, *kind != sim::PropKind::Trees, false)
            }
            _ => continue,
        };
        // Scenery has no view: it burns where its cell is.
        let point = match event {
            SimEvent::PropDestroyed { cell, .. } => {
                Some((crate::projection::cell_centre(*cell, map.tile), *cell))
            }
            _ => points.get(&id).copied(),
        };
        let Some((pos, cell)) = point else {
            continue;
        };
        if !vision.is_visible(cell) {
            continue;
        }
        let amount = if shot {
            3
        } else if large {
            36
        } else {
            18
        };
        let radius = if large {
            50.0
        } else if shot {
            8.0
        } else {
            22.0
        };
        for i in 0..amount.min(budget) {
            let angle = i as f32 * 2.399 + id.0 as f32;
            let velocity = Vec2::new(angle.cos(), angle.sin())
                * (if shot {
                    8.0
                } else {
                    25.0 + (i % 5) as f32 * 8.0
                });
            let smoke = !shot && i % 3 == 0;
            puff(
                &mut commands,
                if smoke { &mat.smoke } else { &mat.glow },
                cell,
                pos,
                velocity,
                if smoke { radius * 0.7 } else { radius },
                if smoke { 30.0 } else { 12.0 },
                if smoke {
                    2.4
                } else if shot {
                    0.15
                } else {
                    0.45 + (i % 5) as f32 * 0.09
                },
                if smoke {
                    [0.13, 0.15, 0.17, 0.7]
                } else {
                    [1.0, 0.46, 0.08, 0.85]
                },
            );
        }
        budget = budget.saturating_sub(amount);
        if !shot {
            for i in 0..6.min(budget) {
                let a = i as f32 * 2.399;
                puff(
                    &mut commands,
                    &Handle::default(),
                    cell,
                    pos,
                    Vec2::new(a.cos() * 43.0, a.sin() * 27.0),
                    2.5,
                    0.0,
                    0.8,
                    [0.57, 0.51, 0.37, 0.9],
                );
            }
            budget = budget.saturating_sub(6);
        }
        if !shot && budget > 0 {
            puff(
                &mut commands,
                &mat.ring,
                cell,
                pos,
                Vec2::ZERO,
                radius,
                180.0,
                0.65,
                [0.88, 0.76, 0.5, 0.8],
            );
            budget -= 1;
            // Lingering ground soot survives the flash, but remains visibility-gated.
            puff(
                &mut commands,
                &mat.smoke,
                cell,
                pos - Vec2::Y * 18.0,
                Vec2::ZERO,
                radius * 1.5,
                0.0,
                18.0,
                [0.05, 0.065, 0.07, 0.7],
            );
        }
    }
    if clock.time - clock.last_puff < 0.28 {
        return;
    }
    clock.last_puff = clock.time;
    for (view, transform, visibility) in &buildings {
        if *visibility == Visibility::Hidden || budget == 0 {
            continue;
        }
        let Some(b) = host.world().get::<sim::Building>(view.building) else {
            continue;
        };
        let Some(def) = host.definitions().and_then(|d| d.building(b.kind)) else {
            continue;
        };
        let damaged = host
            .world()
            .get::<sim::Health>(view.building)
            .is_some_and(|h| {
                host.definitions()
                    .and_then(|d| d.building_combat(b.kind))
                    .is_some_and(|c| h.hit_points < c.hit_points / 2)
            });
        // Whether power lets the building work (RA2: `sim::PlayerState::powers`).
        let powered = host
            .world()
            .get::<sim::Owner>(view.building)
            .and_then(|o| host.players()?.get(o.0))
            .zip(host.definitions())
            .is_some_and(|(p, d)| p.powers(d, b.kind));
        if damaged
            || matches!(
                def.id.as_str(),
                "power_plant" | "refinery" | "nuclear_reactor" | "war_factory"
            )
        {
            let pos = transform.translation.truncate() + Vec2::new(-9.0, 27.0);
            puff(
                &mut commands,
                &mat.smoke,
                b.footprint.origin,
                pos,
                Vec2::new(7.0, 13.0),
                9.0,
                14.0,
                2.6,
                if damaged {
                    [0.12, 0.13, 0.14, 0.6]
                } else {
                    [0.66, 0.72, 0.71, 0.18]
                },
            );
            budget -= 1;
        }
        if powered && budget > 0 {
            let pos = transform.translation.truncate() + Vec2::new(8.0, 8.0);
            puff(
                &mut commands,
                &mat.glow,
                b.footprint.origin,
                pos,
                Vec2::ZERO,
                12.0,
                0.0,
                0.4,
                [0.22, 0.68, 1.0, 0.22],
            );
            budget -= 1;
        }
    }
}
fn animate_particles(
    map: Option<Res<MapView>>,
    mut commands: Commands,
    speed: Res<crate::game_speed::GameClock>,
    sink: Res<CommandSink>,
    host: Res<SimHost>,
    mut particles: Query<(
        Entity,
        &mut Particle,
        &mut Sprite,
        &mut Transform,
        &mut Visibility,
    )>,
) {
    // Particles show gameplay (impacts, smoke), so they age at the game speed.
    let dt = if sink.paused() { 0.0 } else { speed.delta() };
    for (entity, mut p, mut sprite, mut transform, mut visibility) in &mut particles {
        p.age += dt;
        if p.age >= p.life || !host.running() {
            commands.entity(entity).despawn();
            continue;
        }
        let seen = crate::fog::human_vision(host.players()).is_some_and(|v| {
            v.is_visible(p.cell)
                && map
                    .as_ref()
                    .and_then(|map| {
                        crate::projection::pick_cell(
                            transform.translation.truncate(),
                            map.tile,
                            map.width,
                            map.height,
                        )
                    })
                    .is_some_and(|cell| v.is_visible(cell))
        });
        *visibility = if seen {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        transform.translation += (p.velocity * dt).extend(0.0);
        sprite.custom_size = Some(Vec2::splat(p.size + p.growth * p.age));
        let fade = (1.0 - p.age / p.life).powf(1.3);
        sprite.color = Color::srgba(p.color[0], p.color[1], p.color[2], p.color[3] * fade);
    }
}
/// Called with match teardown, including restart and changing maps. No terrain or particles leak.
pub(crate) fn clear(world: &mut World) {
    let entities: Vec<_> = world
        .query_filtered::<Entity, With<BattlefieldVisual>>()
        .iter(world)
        .collect();
    for entity in entities {
        world.despawn(entity);
    }
    world.insert_resource(AtmosphereClock::default());
}

#[cfg(test)]
pub(crate) fn privacy_prepare_order_map(app: &mut App) {
    use bevy::ecs::system::RunSystemOnce;
    privacy_materials(app);
    app.world_mut().run_system_once(terrain).unwrap();
}

#[cfg(test)]
pub(crate) fn privacy_reconcile_order_map(app: &mut App) {
    use bevy::ecs::system::RunSystemOnce;
    app.world_mut().run_system_once(bridge_states).unwrap();
    app.world_mut().run_system_once(raze_scenery).unwrap();
}

#[cfg(test)]
mod tests {
    include!("privacy_scenery_tests.rs");
    use bevy::ecs::system::RunSystemOnce;
    use sim::{Command, Crates, Grid, Mobile, Owner, PlayerId};

    use super::*;
    use crate::sim_host::tests::{demo_host_with, run};

    const HUMAN: PlayerId = PlayerId(0);

    fn view(host: &SimHost) -> MapView {
        let grid = host.grid().expect("a grid");
        MapView {
            width: grid.width(),
            height: grid.height(),
            tile: crate::projection::TileSize::new(64, 32),
        }
    }

    /// A unit of the human player and a free cell of building ground beside it.
    fn unit_and_free_cell(world: &mut World) -> (SimId, CellPos) {
        let mut units = world.query::<(&SimId, &Owner, &Mobile)>();
        let mut own: Vec<(SimId, CellPos)> = units
            .iter(world)
            .filter(|(_, owner, _)| owner.0 == HUMAN)
            .map(|(id, _, mobile)| (*id, mobile.cell))
            .collect();
        own.sort();
        let grid = world.resource::<Grid>();
        let crates = world.resource::<Crates>();
        own.into_iter()
            .find_map(|(id, at)| {
                [(1, 0), (-1, 0), (0, 1), (0, -1)]
                    .into_iter()
                    .map(|(dx, dy)| CellPos::new(at.x + dx, at.y + dy))
                    .find(|&cell| {
                        sim::crate_fits(grid, crates, cell)
                            && grid.cell(cell).is_some_and(|terrain| terrain.buildable)
                    })
                    .map(|cell| (id, cell))
            })
            .expect("a free cell beside a unit of the human player")
    }

    fn marks_at(app: &mut App, cell: CellPos) -> usize {
        let world = app.world_mut();
        let mut marks = world.query::<&CrateMark>();
        marks.iter(world).filter(|mark| mark.0 == cell).count()
    }

    /// RA5: a crate the human player sees is marked (two sprites), and the mark goes when the
    /// crate is taken.
    #[test]
    fn a_crate_in_sight_is_marked_until_it_is_taken() {
        let mut placed = None;
        let host = demo_host_with(|world| {
            let (unit, cell) = unit_and_free_cell(world);
            let grid = world.resource::<Grid>().clone();
            assert!(world.resource_mut::<Crates>().place(&grid, cell));
            placed = Some((unit, cell));
        });
        let (unit, cell) = placed.expect("a crate");
        // Before the first tick nothing is visible, and nothing is marked.
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let map = view(&host);
        app.world_mut().insert_resource(map);
        app.world_mut().insert_resource(host);
        app.world_mut().run_system_once(show_crates).expect("runs");
        assert_eq!(marks_at(&mut app, cell), 0);
        // The simulation is taken out of the application to be advanced, and put back.
        let advance = |app: &mut App, commands: Vec<Command>| {
            let mut host = app
                .world_mut()
                .remove_resource::<SimHost>()
                .expect("the simulation");
            run(&mut host, commands);
            let lies = host.world().resource::<Crates>().contains(cell);
            app.world_mut().insert_resource(host);
            lies
        };
        // After a tick the unit beside it sees the cell.
        assert!(advance(&mut app, Vec::new()));
        app.world_mut().run_system_once(show_crates).expect("runs");
        assert_eq!(marks_at(&mut app, cell), 2);
        // Running again does not mark it twice.
        app.world_mut().run_system_once(show_crates).expect("runs");
        assert_eq!(marks_at(&mut app, cell), 2);
        // The unit drives onto it: the crate is gone from the simulation and from the picture.
        let mut lies = advance(
            &mut app,
            vec![Command::Move {
                units: vec![unit],
                target: cell,
            }],
        );
        for _ in 0..200 {
            if !lies {
                break;
            }
            lies = advance(&mut app, Vec::new());
        }
        assert!(!lies, "the unit took the crate");
        app.world_mut().run_system_once(show_crates).expect("runs");
        assert_eq!(marks_at(&mut app, cell), 0);
    }

    /// RA5: ore on ground without an ore top (ore that spread) gets one top, once, hidden until
    /// the player sees the cell.
    #[test]
    fn ore_on_new_ground_gets_one_ore_top() {
        let mut planted = None;
        let host = demo_host_with(|world| {
            let (_, cell) = unit_and_free_cell(world);
            let terrain = *world.resource::<Grid>().cell(cell).expect("a cell");
            world
                .resource_mut::<Grid>()
                .set_terrain(cell, sim::Cell { ore: 3, ..terrain });
            planted = Some(cell);
        });
        let cell = planted.expect("ore");
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let map = view(&host);
        app.world_mut().insert_resource(map);
        app.world_mut().insert_resource(Materials {
            terrain: vec![Handle::default(); ATLASES],
            glow: Handle::default(),
            smoke: Handle::default(),
            ring: Handle::default(),
            crate_image: Handle::default(),
        });
        app.world_mut().insert_resource(host);
        let tops = |app: &mut App| {
            let world = app.world_mut();
            let mut ground = world.query::<&Ground>();
            ground
                .iter(world)
                .filter(|tile| tile.ore && tile.cell == cell)
                .map(|tile| tile.depleted)
                .collect::<Vec<bool>>()
        };
        assert!(tops(&mut app).is_empty());
        // Tick 0 is a tick of the search.
        app.world_mut().run_system_once(sprout_ore).expect("runs");
        assert_eq!(tops(&mut app), [true], "one top, hidden until it is seen");
        app.world_mut().run_system_once(sprout_ore).expect("runs");
        assert_eq!(tops(&mut app).len(), 1, "not twice");
    }

    /// Both map-authored and newly discovered gems use the ore top with their own tint.
    #[test]
    fn gems_have_a_distinct_tint_in_both_terrain_paths() {
        for sprouted in [false, true] {
            let host = demo_host_with(|world| {
                let mut grid = Grid::from_cells(
                    2,
                    1,
                    vec![
                        sim::Cell {
                            passable: true,
                            water: false,
                            buildable: false,
                            ore: 3,
                        };
                        2
                    ],
                )
                .expect("two ore cells");
                assert!(grid.set_gems(CellPos::new(1, 0), true));
                world.insert_resource(grid);
            });
            let mut app = App::new();
            app.add_plugins(MinimalPlugins);
            app.world_mut().insert_resource(view(&host));
            app.world_mut()
                .insert_resource(MapTerrain(host.grid().expect("a grid").clone()));
            app.world_mut().insert_resource(crate::map::MapLooks(vec![
                looks::look(Soil::Grass, Feature::Ore),
                looks::look(Soil::Grass, Feature::Gems),
            ]));
            app.world_mut().insert_resource(Materials {
                terrain: vec![Handle::default(); ATLASES],
                glow: Handle::default(),
                smoke: Handle::default(),
                ring: Handle::default(),
                crate_image: Handle::default(),
            });
            app.world_mut().insert_resource(host);
            if sprouted {
                app.world_mut().run_system_once(sprout_ore).expect("runs");
            } else {
                app.world_mut().run_system_once(terrain).expect("runs");
            }
            let world = app.world_mut();
            let mut query = world.query::<(&Ground, &Sprite)>();
            let mut tops: Vec<_> = query
                .iter(world)
                .filter(|(tile, _)| tile.ore)
                .map(|(tile, sprite)| (tile.cell.x, tile.stain, sprite.color))
                .collect();
            tops.sort_by_key(|top| top.0);
            assert_eq!(tops.len(), 2);
            assert_eq!(tops[0].1, [1.0, 1.0, 1.0]);
            assert_eq!(tops[1].1, [0.42, 1.0, 0.96]);
            if sprouted {
                assert!(tops.iter().all(|top| top.2 == Color::NONE));
            }
        }
    }
}
