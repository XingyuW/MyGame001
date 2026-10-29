//! How new buildings and units arrive on the field. A building placed during the match (or a
//! deployed Construction Yard) rises from its foundation in a cloud of dust instead of appearing
//! whole; a unit that leaves the building that produced it comes out of it instead of appearing
//! beside it. A building whose art has a vehicle door (the war factory, `scripts/hd_buildings.py`)
//! rolls the door up, and the vehicle drives out of the dark hall through it onto the apron; any
//! other building lets the unit step out of its side.
//!
//! As for the rest of the presentation (`crate::units`), the simulation is only read: the unit
//! already stands on its exit cell when `SimEvent::UnitSpawned` arrives, so its view is drawn
//! along the way out for its first moments, and then catches up with it.
//!
//! The doorway of an open door frame is cut out of the sprite (a holdout in the render), and a
//! vehicle inside is drawn behind the building: it is seen only through the doorway, behind the
//! door as it rolls up, until it has crossed the wall.

use std::collections::{BTreeMap, BTreeSet};
use std::f32::consts::{FRAC_PI_4, SQRT_2};

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use sim::{Building, BuildingKind, Direction, Footprint, Position, SimEvent, SimId};

use crate::buildings::{BuildingView, BuildingViews, building_depth};
use crate::map::MapView;
use crate::projection::{TileSize, to_world};
use crate::sim_host::{CommandSink, SimHost};
use crate::units::{UnitView, UnitViews, interpolate, unit_depth};

pub struct ArrivalPlugin;

impl Plugin for ArrivalPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, make_art)
            .add_systems(
                Update,
                (start_arrivals, advance_arrivals)
                    .chain()
                    .after(crate::units::spawn_unit_views)
                    .after(crate::buildings::spawn_building_views),
            )
            .add_systems(
                PostUpdate,
                raise_buildings
                    .after(crate::buildings::update_building_views)
                    .before(bevy::transform::TransformSystems::Propagate),
            );
    }
}

/// World pixels of height per metre of a model (`scripts/hd_kit.py`: 0.1 cells per metre, and
/// `ZC` pixels per cell of height).
pub const PX_PER_METRE: f32 = 0.1 * 32.0 * SQRT_2 * 0.866_025_4;

/// A door modelled in a building's art (a vehicle door, an entrance), in cells of its
/// footprint's own frame. It is in the wall facing +y, which the camera sees, and units leave
/// through it towards +y.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DoorArt {
    /// The art that has it (`Definitions::art`).
    pub art: &'static str,
    /// Middle of the doorway along x, and the y of its wall.
    pub centre: f32,
    pub wall: f32,
    pub width: f32,
    /// Height of the doorway and of its sill above the ground, in world pixels.
    pub height: f32,
    pub sill: f32,
    /// Cells a unit waits inside the doorway and walks or drives beyond it, and how far beyond
    /// the wall it is drawn in front of the building: a soldier at a narrow entrance has to
    /// stand close behind it to be seen through it, a tank in a wide door further back.
    pub inside: f32,
    pub apron: f32,
    pub clear: f32,
    /// Whether a unit inside is seen through the doorway, drawn behind the building. Soldiers,
    /// drawn far larger than the building's scale, would not fit through a real entrance: they
    /// step out in front of the open doorway instead, coming out of its dark.
    pub through: bool,
}

/// Every door: `scripts/hd_buildings.py` (`WF_DOOR_X0`, `WF_DOOR_W`, `WF_DOOR_H` and
/// `WF_WALL_Y` of the war factory, `BK_...` of the barracks, in metres; a cell is 10 m), on the
/// pad's 0.16 m.
pub const DOORS: [DoorArt; 2] = [
    DoorArt {
        art: "war_factory",
        centre: 1.15,
        wall: 1.9,
        width: 0.8,
        height: 6.4 * PX_PER_METRE,
        sill: 0.16 * PX_PER_METRE,
        inside: 0.45,
        apron: 0.85,
        clear: 0.2,
        through: true,
    },
    DoorArt {
        art: "barracks",
        centre: 0.6,
        wall: 0.9,
        width: 0.28,
        height: 2.7 * PX_PER_METRE,
        sill: 0.16 * PX_PER_METRE,
        inside: -0.04,
        apron: 0.6,
        clear: 0.0,
        through: false,
    },
];

/// Frames of a door opening (`<art>_door<n>.png`, n from 1): the last is wide open.
pub const DOOR_FRAMES: usize = 3;

/// The vehicle door of the art `art`, if it has one.
pub fn door_art(art: &str) -> Option<&'static DoorArt> {
    DOORS.iter().find(|door| door.art == art)
}

/// The sprite (under `sprites/hd/buildings`) of frame `frame` (from 1) of the door of `art`.
pub fn door_sprite(art: &str, frame: usize) -> String {
    format!("{art}_door{frame}")
}

/// The door frames of every building type whose art has a door: the human player's copy and
/// the first opponent's of each, as `crate::buildings::BuildingSprites`.
#[derive(Resource, Debug, Default)]
pub struct DoorSprites(pub BTreeMap<BuildingKind, Vec<DoorFrame>>);

/// A door frame: the human player's copy and the first opponent's.
pub type DoorFrame = (Handle<Image>, Handle<Image>);

/// Seconds of game time a door takes to roll up or down.
pub const DOOR_SECONDS: f32 = 0.4;
/// The way out of a door: the vehicle waits inside while the door rolls up, drives out onto the
/// apron, then joins the unit where the simulation has it by then.
const DRIVE_FROM: f32 = DOOR_SECONDS;
const DRIVE_TO: f32 = 1.25;
const CATCH_UP: f32 = 1.65;
/// How long a door stays open after a vehicle started out of it.
const DOOR_HOLD: f32 = DRIVE_TO + 0.2;
/// How far behind the building a vehicle inside it, and the hall behind that, are drawn.
const BEHIND: f32 = 0.4 / 1024.0;
/// Seconds a unit takes to step out of the side of a building without a door.
const SIDE_SECONDS: f32 = 0.6;

/// A building view rising from its foundation: the building was placed during the match.
#[derive(Component, Debug, Default)]
pub struct Rising {
    age: f32,
    /// Where and how large the view is drawn once it stands (read on its first frame).
    rest: Option<(Vec3, Vec2)>,
    /// Puffs of dust thrown up so far.
    puffs: u32,
}

/// Seconds a building of `footprint` takes to rise: a larger one takes longer.
pub fn rise_seconds(footprint: Footprint) -> f32 {
    0.9 + 0.2 * f32::from(footprint.width + footprint.height)
}

/// The vehicle door of a building view: how far it is open (0 to 1), and how much longer it
/// stays open for the vehicles driving out.
#[derive(Component, Debug, Default)]
pub struct Door {
    pub open: f32,
    hold: f32,
}

impl Door {
    /// The door frame shown: 0 shut (the building's own sprite) up to [`DOOR_FRAMES`].
    pub fn frame(&self) -> usize {
        (self.open * DOOR_FRAMES as f32).round() as usize
    }
}

/// The dark hall seen through an open door: a child of the building view, drawn behind it.
#[derive(Component, Debug)]
struct Hall;

/// A unit view on its way out of the building that produced it.
#[derive(Component, Debug)]
pub struct Emerging {
    age: f32,
    way: Way,
    /// Whether it has thrown up dust where it crossed the door's sill.
    dusted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Way {
    /// Out of a door: from inside the building through the wall at `wall` (the cell y of the
    /// door's wall) to the apron in front of it, in front of the building from `clear` beyond
    /// the wall; `depth` is the building's. A unit not seen `through` the doorway is drawn in
    /// front of the building until it has left the footprint, whose front edge is `edge`.
    Door {
        inside: Vec2,
        wall: f32,
        apron: Vec2,
        clear: f32,
        depth: f32,
        through: bool,
        edge: f32,
    },
    /// Out of the side of a building without a door, from the point of its edge nearest to
    /// where the unit stands.
    Side { from: Vec2 },
}

/// Where and how a unit view on its way out is drawn this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// Its ground point, in continuous cell coordinates.
    pub cell: Vec2,
    /// The way it faces instead of its unit's facing.
    pub facing: Option<Direction>,
    /// The depth it is drawn at instead of its own: behind the building, seen through the door.
    pub depth: Option<f32>,
    /// Its brightness (dark in the hall) and opacity.
    pub light: f32,
    pub alpha: f32,
}

impl Pose {
    pub fn colour(&self) -> Color {
        Color::srgba(self.light, self.light, self.light, self.alpha)
    }
}

/// Eased 0 to 1 over `x` from 0 to 1.
fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// The facing nearest to moving along `delta` (cells); `None` for a step too short to tell.
pub fn facing_along(delta: Vec2) -> Option<Direction> {
    if delta.length() < 0.1 {
        return None;
    }
    // Direction::ALL turns clockwise from east in steps of 45°, as the angle of +y from +x does.
    let sector = (delta.y.atan2(delta.x) / FRAC_PI_4).round() as i32;
    Some(Direction::ALL[sector.rem_euclid(8) as usize])
}

impl Emerging {
    /// A vehicle leaving the building of `footprint` through `door`.
    pub fn through(door: &DoorArt, footprint: Footprint) -> Self {
        let (x, y) = (footprint.origin.x as f32, footprint.origin.y as f32);
        let way = Way::Door {
            inside: Vec2::new(x + door.centre, y + door.wall - door.inside),
            wall: y + door.wall,
            apron: Vec2::new(x + door.centre, y + door.wall + door.apron),
            clear: door.clear,
            depth: building_depth(footprint),
            through: door.through,
            edge: y + f32::from(footprint.height),
        };
        Self {
            age: 0.0,
            way,
            // Only tracks and wheels throw up dust on the sill.
            dusted: !door.through,
        }
    }

    /// A unit stepping out of the side of the building of `footprint` towards `cell`, where it
    /// stands.
    pub fn beside(footprint: Footprint, cell: Vec2) -> Self {
        let (x, y) = (footprint.origin.x as f32, footprint.origin.y as f32);
        let corner = Vec2::new(
            x + f32::from(footprint.width),
            y + f32::from(footprint.height),
        );
        Self {
            age: 0.0,
            way: Way::Side {
                from: cell.clamp(Vec2::new(x, y), corner),
            },
            dusted: true,
        }
    }

    pub fn done(&self) -> bool {
        self.age
            >= match self.way {
                Way::Door { .. } => CATCH_UP,
                Way::Side { .. } => SIDE_SECONDS,
            }
    }

    /// How the view is drawn while the simulation has its unit at `cell`.
    pub fn pose(&self, cell: Vec2) -> Pose {
        let t = self.age;
        match self.way {
            Way::Door {
                inside,
                wall,
                apron,
                clear,
                depth,
                through,
                edge,
            } => {
                let (at, facing) = if t < DRIVE_TO {
                    let k = smooth((t - DRIVE_FROM) / (DRIVE_TO - DRIVE_FROM));
                    (inside.lerp(apron, k), Some(Direction::South))
                } else {
                    let k = smooth((t - DRIVE_TO) / (CATCH_UP - DRIVE_TO));
                    (apron.lerp(cell, k), facing_along(cell - apron))
                };
                if through {
                    // Seen through the doorway until it is out, lit as it leaves the hall.
                    Pose {
                        cell: at,
                        facing,
                        depth: (at.y < wall + clear).then_some(depth - BEHIND),
                        light: 1.0
                            - 0.7 * (1.0 - smooth((at.y - inside.y) / (wall + clear - inside.y))),
                        alpha: 1.0,
                    }
                } else {
                    // In front of the open doorway, coming out of its dark.
                    let out = (t - DRIVE_FROM) / (DRIVE_TO - DRIVE_FROM);
                    Pose {
                        cell: at,
                        facing,
                        depth: (at.y < edge).then_some(depth + BEHIND),
                        light: 1.0 - 0.65 * (1.0 - smooth(out / 0.7)),
                        alpha: smooth(out / 0.35),
                    }
                }
            }
            Way::Side { from } => Pose {
                cell: from.lerp(cell, smooth(t / SIDE_SECONDS)),
                facing: facing_along(cell - from),
                depth: None,
                light: 1.0,
                alpha: smooth(t / (SIDE_SECONDS * 0.6)),
            },
        }
    }
}

/// A puff of dust: thrown up by a building rising and by a vehicle crossing a door's sill.
#[derive(Component, Debug)]
struct Dust {
    age: f32,
    life: f32,
    velocity: Vec2,
    size: (f32, f32),
    alpha: f32,
}

/// The textures of the arrivals, drawn once at startup.
#[derive(Resource)]
struct ArrivalArt {
    /// A soft round blob.
    puff: Handle<Image>,
    /// The inside of a hall seen through its doorway: dark under the roof, a little light on
    /// the floor near the door.
    hall: Handle<Image>,
}

fn texture(size: u32, pixel: impl Fn(f32, f32) -> [u8; 4]) -> Image {
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (u, v) = (
                (x as f32 + 0.5) / size as f32,
                (y as f32 + 0.5) / size as f32,
            );
            data.extend_from_slice(&pixel(u, v));
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    image
}

fn make_art(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let puff = texture(32, |u, v| {
        let r2 = (u * 2.0 - 1.0).powi(2) + (v * 2.0 - 1.0).powi(2);
        let alpha = (-r2 * 3.2).exp() * (1.0 - r2).max(0.0);
        [255, 255, 255, (alpha * 255.0) as u8]
    });
    let hall = texture(32, |_, v| {
        // Top (v = 0) to bottom: the dark roof space, then the lit floor.
        let light = 10.0 + 34.0 * v.powf(2.2);
        [light as u8, (light * 0.94) as u8, (light * 0.84) as u8, 255]
    });
    commands.insert_resource(ArrivalArt {
        puff: images.add(puff),
        hall: images.add(hall),
    });
}

/// The building of the simulation with `id`, if it still stands.
fn building(host: &SimHost, id: SimId) -> Option<Building> {
    let mut buildings = host.world().try_query::<(&SimId, &Building)>()?;
    buildings
        .iter(host.world())
        .find(|(found, _)| **found == id)
        .map(|(_, building)| building.clone())
}

/// Where the middle of the doorway of `door` in a building of `footprint` is drawn, relative to
/// the building's view, and how large a rectangle covers the doorway.
pub fn hall_place(door: &DoorArt, footprint: Footprint, tile: TileSize) -> (Vec2, Vec2) {
    let fp = |x: f32, y: f32| {
        to_world(
            footprint.origin.x as f32 + x,
            footprint.origin.y as f32 + y,
            tile,
        )
    };
    let middle = fp(door.centre, door.wall) + Vec2::Y * (door.sill + door.height / 2.0);
    let size = Vec2::new(
        door.width * tile.width / 2.0,
        door.width * tile.height / 2.0 + door.height,
    );
    (
        middle - crate::hd_art::building_position(footprint, tile),
        size,
    )
}

/// Starts the arrival of every building placed and every unit that left a building since the
/// last frame. Runs after the views of new buildings and units are created.
#[allow(clippy::too_many_arguments)]
fn start_arrivals(
    mut events: MessageReader<SimEvent>,
    host: Res<SimHost>,
    map: Option<Res<MapView>>,
    art: Option<Res<ArrivalArt>>,
    doors: Option<Res<DoorSprites>>,
    building_views: Res<BuildingViews>,
    unit_views: Res<UnitViews>,
    mut open: Query<&mut Door, With<BuildingView>>,
    mut commands: Commands,
) {
    let mut opened = BTreeSet::new();
    for event in events.read() {
        match *event {
            SimEvent::BuildingPlaced { building, .. } => {
                if let Some(&view) = building_views.0.get(&building) {
                    commands.entity(view).insert(Rising::default());
                }
            }
            SimEvent::UnitSpawned { unit, building, .. } => {
                let (Some(&view), Some(from)) =
                    (unit_views.0.get(&unit), self::building(&host, building))
                else {
                    continue;
                };
                let flying = host
                    .grid()
                    .is_some_and(|grid| grid.domain_of(unit) == sim::Domain::Air);
                let door = host
                    .definitions()
                    .filter(|_| !flying)
                    .filter(|_| doors.as_ref().is_some_and(|d| d.0.contains_key(&from.kind)))
                    .and_then(|defs| door_art(defs.art(sim::ProductionItem::Building(from.kind))));
                let Some(door) = door else {
                    let cell =
                        host.world()
                            .try_query::<(&SimId, &Position)>()
                            .and_then(|mut units| {
                                units
                                    .iter(host.world())
                                    .find(|(id, _)| **id == unit)
                                    .map(|(_, position)| interpolate(position.0, position.0, 1.0))
                            });
                    if let Some(cell) = cell {
                        commands
                            .entity(view)
                            .insert(Emerging::beside(from.footprint, cell));
                    }
                    continue;
                };
                commands
                    .entity(view)
                    .insert(Emerging::through(door, from.footprint));
                // Open the factory's door, and give it its hall the first time.
                let Some(&factory) = building_views.0.get(&building) else {
                    continue;
                };
                if let Ok(mut state) = open.get_mut(factory) {
                    state.hold = state.hold.max(DOOR_HOLD);
                } else if opened.insert(factory)
                    && let (Some(map), Some(art)) = (map.as_ref(), art.as_ref())
                {
                    let (offset, size) = hall_place(door, from.footprint, map.tile);
                    commands
                        .entity(factory)
                        .insert(Door {
                            open: 0.0,
                            hold: DOOR_HOLD,
                        })
                        .with_child((
                            Hall,
                            Sprite {
                                image: art.hall.clone(),
                                custom_size: Some(size),
                                ..default()
                            },
                            Transform::from_translation(offset.extend(-2.0 * BEHIND)),
                            Visibility::Hidden,
                        ));
                }
            }
            _ => {}
        }
    }
}

/// Advances every arrival by the game time of this frame: the ways out, the doors and the dust.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn advance_arrivals(
    speed: Res<crate::game_speed::GameClock>,
    sink: Res<CommandSink>,
    host: Res<SimHost>,
    map: Option<Res<MapView>>,
    art: Option<Res<ArrivalArt>>,
    mut commands: Commands,
    mut leaving: Query<(Entity, &mut Emerging, &Visibility)>,
    mut doors: Query<(&mut Door, &Children, Has<Rising>)>,
    mut halls: Query<&mut Visibility, (With<Hall>, Without<Emerging>, Without<Dust>)>,
    mut dust: Query<(Entity, &mut Dust, &mut Sprite, &mut Transform), Without<Hall>>,
) {
    let dt = if sink.paused() { 0.0 } else { speed.delta() };
    for (entity, mut emerging, visibility) in &mut leaving {
        emerging.age += dt;
        if emerging.done() {
            commands.entity(entity).remove::<Emerging>();
            continue;
        }
        // The tracks throw up a little dust as the vehicle rolls over the sill.
        if let Way::Door { wall, .. } = emerging.way
            && !emerging.dusted
            && emerging.pose(Vec2::ZERO).cell.y > wall
        {
            emerging.dusted = true;
            if let (Some(map), Some(art)) = (map.as_ref(), art.as_ref())
                && *visibility != Visibility::Hidden
            {
                // Thrown forward off the tracks, in front of the factory.
                let at = emerging.pose(Vec2::ZERO).cell;
                for i in 0..4 {
                    let side = Vec2::new(i as f32 * 0.16 - 0.24, 0.1);
                    let away = Vec2::new(side.x * 2.0, 1.0);
                    puff(&mut commands, art, map.tile, at + side, away, 1.0, i, 0.6);
                }
            }
        }
    }
    for (mut door, children, rising) in &mut doors {
        door.hold = (door.hold - dt).max(0.0);
        let target = if door.hold > 0.0 { 1.0 } else { 0.0 };
        let step = dt / DOOR_SECONDS;
        door.open = if door.open < target {
            (door.open + step).min(target)
        } else {
            (door.open - step).max(target)
        };
        let shown = if door.open > 0.01 && !rising {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        for child in children {
            if let Ok(mut visibility) = halls.get_mut(*child)
                && *visibility != shown
            {
                *visibility = shown;
            }
        }
    }
    for (entity, mut puff, mut sprite, mut transform) in &mut dust {
        puff.age += dt;
        if puff.age >= puff.life || !host.running() {
            commands.entity(entity).despawn();
            continue;
        }
        let t = puff.age / puff.life;
        transform.translation += (puff.velocity * dt * (1.0 - t)).extend(0.0);
        sprite.custom_size = Some(Vec2::splat(
            puff.size.0 + (puff.size.1 - puff.size.0) * smooth(t),
        ));
        sprite.color = Color::srgba(0.66, 0.61, 0.52, puff.alpha * (1.0 - t).powf(1.4));
    }
}

/// A small integer hash, for where the dust goes (presentation only: any spread will do).
fn scatter(seed: u32) -> f32 {
    let mut x = seed.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x % 10_000) as f32 / 10_000.0
}

/// Throws up a puff of dust at `cell` (continuous cell coordinates), blown along `away` (cells)
/// and rising; `ahead` cells along +x and +y decide what it is drawn in front of.
#[allow(clippy::too_many_arguments)]
fn puff(
    commands: &mut Commands,
    art: &ArrivalArt,
    tile: TileSize,
    cell: Vec2,
    away: Vec2,
    ahead: f32,
    seed: u32,
    scale: f32,
) {
    let ground = to_world(cell.x, cell.y, tile);
    let out = to_world(away.x, away.y, tile).normalize_or_zero();
    let speed = 12.0 + 10.0 * scatter(seed ^ 0x33);
    let velocity = (out * speed + Vec2::Y * (5.0 + 5.0 * scatter(seed ^ 0x77))) * scale;
    let depth = unit_depth(cell + Vec2::splat(ahead / 2.0));
    commands.spawn((
        Dust {
            age: 0.0,
            life: 1.3 + 0.9 * scatter(seed ^ 0x55),
            velocity,
            size: (18.0 * scale, (44.0 + 24.0 * scatter(seed ^ 0xAA)) * scale),
            alpha: 0.62,
        },
        Sprite {
            image: art.puff.clone(),
            custom_size: Some(Vec2::splat(18.0 * scale)),
            color: Color::NONE,
            ..default()
        },
        Transform::from_translation((ground + Vec2::Y * 5.0 * scale).extend(depth)),
    ));
}

/// A point of the edge of `footprint` (continuous cell coordinates), `along` from 0 to 1 round
/// it, pushed `out` cells beyond it.
fn edge_point(footprint: Footprint, along: f32, out: f32) -> Vec2 {
    let (w, h) = (f32::from(footprint.width), f32::from(footprint.height));
    let origin = Vec2::new(footprint.origin.x as f32, footprint.origin.y as f32);
    let s = along.rem_euclid(1.0) * 2.0 * (w + h);
    let local = if s < w {
        Vec2::new(s, -out)
    } else if s < w + h {
        Vec2::new(w + out, s - w)
    } else if s < 2.0 * w + h {
        Vec2::new(w - (s - w - h), h + out)
    } else {
        Vec2::new(-out, h - (s - 2.0 * w - h))
    };
    origin + local
}

/// Raises every rising building view: the sprite is revealed from its foot upwards while it
/// grows to its full size, coming out of the dust it throws up; then it stands as any other.
#[allow(clippy::type_complexity)]
fn raise_buildings(
    speed: Res<crate::game_speed::GameClock>,
    sink: Res<CommandSink>,
    host: Res<SimHost>,
    map: Option<Res<MapView>>,
    art: Option<Res<ArrivalArt>>,
    mut commands: Commands,
    mut views: Query<
        (
            Entity,
            &BuildingView,
            &mut Rising,
            &mut Sprite,
            &mut Transform,
            &Visibility,
        ),
        (Without<UnitView>, Without<Dust>, Without<Hall>),
    >,
) {
    let dt = if sink.paused() { 0.0 } else { speed.delta() };
    for (entity, view, mut rising, mut sprite, mut transform, visibility) in &mut views {
        let Some(footprint) = host
            .world()
            .get::<Building>(view.building)
            .map(|building| building.footprint)
        else {
            continue;
        };
        let size = sprite.custom_size.unwrap_or(Vec2::ONE);
        let (rest, full) = *rising.rest.get_or_insert((transform.translation, size));
        rising.age += dt;
        let seconds = rise_seconds(footprint);
        let p = (rising.age / seconds).clamp(0.0, 1.0);
        if p >= 1.0 {
            transform.translation = rest;
            sprite.custom_size = Some(full);
            sprite.rect = None;
            sprite.color = Color::WHITE;
            commands.entity(entity).remove::<Rising>();
            continue;
        }
        let shape = rise_shape(p);
        // The image is revealed upwards from its foot; it grows about its footprint's centre,
        // which lies `BUILDING_RISE` below the image's.
        let texels = full * crate::hd_art::SCALE as f32;
        sprite.rect = Some(Rect::new(
            0.0,
            texels.y * (1.0 - shape.reveal),
            texels.x,
            texels.y,
        ));
        let shown = Vec2::new(full.x, full.y * shape.reveal) * shape.scale;
        sprite.custom_size = Some(shown);
        let pivot = rest.y - crate::hd_art::BUILDING_RISE;
        let foot = pivot + (rest.y - full.y / 2.0 - pivot) * shape.scale;
        transform.translation = Vec3::new(rest.x, foot + shown.y / 2.0, rest.z);
        let dusty = shape.dust;
        sprite.color = Color::srgb(1.0 - 0.22 * dusty, 1.0 - 0.24 * dusty, 1.0 - 0.28 * dusty);
        // Dust all round the foundation, most of it at the start.
        let total = 8 + 4 * u32::from(footprint.width + footprint.height);
        let due = (total as f32 * (p / 0.7).min(1.0).sqrt()).ceil() as u32;
        if let (Some(map), Some(art)) = (map.as_ref(), art.as_ref())
            && *visibility != Visibility::Hidden
        {
            while rising.puffs < due {
                let seed = (entity.to_bits() as u32)
                    .wrapping_mul(97)
                    .wrapping_add(rising.puffs);
                let at = edge_point(footprint, scatter(seed), 0.1);
                let centre = Vec2::new(
                    footprint.origin.x as f32 + f32::from(footprint.width) / 2.0,
                    footprint.origin.y as f32 + f32::from(footprint.height) / 2.0,
                );
                puff(
                    &mut commands,
                    art,
                    map.tile,
                    at,
                    at - centre,
                    0.0,
                    seed,
                    1.0,
                );
                rising.puffs += 1;
            }
        } else {
            rising.puffs = rising.puffs.max(due);
        }
    }
}

/// How far a building has risen at `p` (0 to 1) of its time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RiseShape {
    /// Share of the image shown, from its foot.
    pub reveal: f32,
    /// Size against its standing size: it grows, and settles after a slight overshoot.
    pub scale: f32,
    /// How much of the construction dust still dulls it.
    pub dust: f32,
}

pub fn rise_shape(p: f32) -> RiseShape {
    let p = p.clamp(0.0, 1.0);
    // Ease out with a little overshoot ("back"), from 0 to 1.
    let (c1, q) = (1.70158, p - 1.0);
    let back = 1.0 + (c1 + 1.0) * q * q * q + c1 * q * q;
    RiseShape {
        reveal: smooth(p / 0.85).max(0.002),
        scale: 1.0 - 0.12 * (1.0 - back),
        dust: 1.0 - smooth(p),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::CellPos;

    fn factory() -> Footprint {
        Footprint {
            origin: CellPos::new(10, 20),
            width: 3,
            height: 3,
        }
    }

    /// The doors' numbers here are those of the models the sprites were rendered from.
    #[test]
    fn every_door_is_where_its_model_has_it() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts");
        let model = std::fs::read_to_string(root.join("hd_buildings.py")).expect("model");
        for (prefix, art) in [("WF", "war_factory"), ("BK", "barracks")] {
            let names =
                format!("{prefix}_DOOR_X0, {prefix}_DOOR_W, {prefix}_DOOR_H, {prefix}_WALL_Y = ");
            let line = model
                .lines()
                .find(|line| line.starts_with(&names))
                .expect("door line");
            let metres: Vec<f32> = line[names.len()..]
                .split(',')
                .map(|v| v.trim().parse().expect("number"))
                .collect();
            let door = door_art(art).expect("door");
            let (x0, w, h, wall) = (metres[0], metres[1], metres[2], metres[3]);
            assert!(((x0 + w / 2.0) / 10.0 - door.centre).abs() < 1e-4, "{art}");
            assert!((w / 10.0 - door.width).abs() < 1e-4, "{art}");
            assert!((wall / 10.0 - door.wall).abs() < 1e-4, "{art}");
            assert!((h * PX_PER_METRE - door.height).abs() < 1e-3, "{art}");
            assert!(
                model.contains(&format!("'{art}': (1 / 3, 2 / 3, 1.0)")),
                "{art}"
            );
        }
        assert_eq!(DOOR_FRAMES, 3);
    }

    /// The shipped door frames open the doorway the game draws its hall and units behind: it is
    /// solid in the building's own sprite, and see-through once the door is open. The war
    /// factory's door rolls up from the floor.
    #[test]
    fn every_open_doorway_is_cut_out_of_the_sprite() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let load = |name: &str| {
            let path = root.join(format!("sprites/hd/buildings/{name}.png"));
            image::open(&path).expect("sprite").to_rgba8()
        };
        for (door, cells) in [(&DOORS[0], 3.0), (&DOORS[1], 2.0)] {
            // Two points low in the doorway's left quarter (an open leaf may swing across its
            // middle, a canopy hide its top), in texels of the sprite: the model point (x, y, z)
            // is drawn at (16 + 32 d + 32 (x - y), 88 + 16 (x + y) - z) world pixels.
            let (x, y) = (door.centre - door.width / 4.0, door.wall);
            let texel = |z: f32| {
                let s = crate::hd_art::SCALE as f32;
                (
                    ((16.0 + 32.0 * cells + 32.0 * (x - y)) * s) as u32,
                    ((88.0 + 16.0 * (x + y) - z) * s) as u32,
                )
            };
            let low = texel(door.sill + 0.2 * door.height);
            let middle = texel(door.sill + 0.4 * door.height);
            let alpha = |image: &image::RgbaImage, (x, y): (u32, u32)| image.get_pixel(x, y)[3];
            let shut = load(door.art);
            assert_eq!(alpha(&shut, low), 255, "{}", door.art);
            assert_eq!(alpha(&shut, middle), 255, "{}", door.art);
            let open = load(&door_sprite(door.art, DOOR_FRAMES));
            assert_eq!(alpha(&open, low), 0, "{}", door.art);
            assert_eq!(alpha(&open, middle), 0, "{}", door.art);
            for frame in 1..=DOOR_FRAMES {
                assert_eq!(
                    load(&door_sprite(door.art, frame)).dimensions(),
                    shut.dimensions()
                );
            }
        }
        let first = load(&door_sprite("war_factory", 1));
        let door = door_art("war_factory").expect("door");
        let s = crate::hd_art::SCALE as f32;
        let at = |z: f32| {
            let (x, y) = (door.centre, door.wall);
            first.get_pixel(
                ((112.0 + 32.0 * (x - y)) * s) as u32,
                ((88.0 + 16.0 * (x + y) - door.sill - z * door.height) * s) as u32,
            )[3]
        };
        assert_eq!(
            (at(0.2), at(0.6)),
            (0, 255),
            "the door rolls up from the floor"
        );
    }

    /// A vehicle waits in the dark behind the door while it opens, drives out through the
    /// doorway in front of the building onto the apron, and ends where its unit is.
    #[test]
    fn a_vehicle_drives_out_of_the_door_and_joins_its_unit() {
        let door = door_art("war_factory").expect("door");
        let mut leaving = Emerging::through(door, factory());
        let wall = 20.0 + door.wall;
        let unit = Vec2::new(13.5, 23.5);
        let start = leaving.pose(unit);
        assert_eq!(
            start.cell,
            Vec2::new(10.0 + door.centre, wall - door.inside)
        );
        assert_eq!(start.facing, Some(Direction::South));
        assert!(start.depth.expect("behind") < building_depth(factory()));
        assert!(start.light < 0.4);
        let mut previous = start.cell.y;
        let mut crossed = false;
        while !leaving.done() {
            leaving.age += 1.0 / 60.0;
            let pose = leaving.pose(unit);
            if leaving.age < DRIVE_TO {
                assert!(pose.cell.y >= previous, "it only drives out");
                previous = pose.cell.y;
                assert!((pose.cell.x - (10.0 + door.centre)).abs() < 1e-4);
            }
            if pose.cell.y > wall + door.clear {
                crossed = true;
                assert_eq!(pose.depth, None, "in front once out of the door");
                assert_eq!(pose.light, 1.0);
            }
        }
        assert!(crossed);
        assert!(leaving.pose(unit).cell.distance(unit) < 1e-5);
        // Before it has crossed the wall, the door's own sprite hides it.
        let half = Emerging {
            age: DRIVE_FROM * 0.5,
            ..Emerging::through(door, factory())
        };
        assert!(half.pose(unit).depth.is_some());
    }

    /// A soldier steps out in front of the training centre's open entrance, out of its dark,
    /// never hidden behind the building, and walks on to where its unit stands.
    #[test]
    fn a_soldier_walks_out_of_the_entrance() {
        let door = door_art("barracks").expect("entrance");
        let barracks = Footprint {
            width: 2,
            height: 2,
            ..factory()
        };
        let mut leaving = Emerging::through(door, barracks);
        let unit = Vec2::new(10.5, 22.5);
        let start = leaving.pose(unit);
        assert_eq!(start.alpha, 0.0, "unseen while the doors open");
        assert_eq!(start.facing, Some(Direction::South));
        let front = building_depth(barracks);
        let mut seen = false;
        while !leaving.done() {
            leaving.age += 1.0 / 60.0;
            let pose = leaving.pose(unit);
            if pose.cell.y < 22.0 {
                assert!(
                    pose.depth.expect("in front") > front,
                    "never behind the building"
                );
            }
            seen |= pose.alpha > 0.5 && pose.light < 0.9;
        }
        assert!(seen, "it comes out of the dark doorway");
        let end = leaving.pose(unit);
        assert!(end.cell.distance(unit) < 1e-5);
        assert_eq!((end.alpha, end.light), (1.0, 1.0));
    }

    /// Without a door, a unit fades in at the building's edge and walks to where it stands.
    #[test]
    fn a_unit_steps_out_of_the_side_of_a_building_without_a_door() {
        let unit = Vec2::new(13.5, 21.5);
        let mut leaving = Emerging::beside(factory(), unit);
        let start = leaving.pose(unit);
        assert_eq!(start.cell, Vec2::new(13.0, 21.5));
        assert_eq!(start.alpha, 0.0);
        assert_eq!(start.facing, Some(Direction::East));
        leaving.age = SIDE_SECONDS;
        assert!(leaving.done());
        let end = leaving.pose(unit);
        assert!(end.cell.distance(unit) < 1e-5);
        assert_eq!((end.alpha, end.depth), (1.0, None));
    }

    #[test]
    fn a_way_faces_the_nearest_of_eight_directions() {
        assert_eq!(facing_along(Vec2::new(1.0, 0.0)), Some(Direction::East));
        assert_eq!(facing_along(Vec2::new(0.0, 1.0)), Some(Direction::South));
        assert_eq!(
            facing_along(Vec2::new(1.0, 1.1)),
            Some(Direction::SouthEast)
        );
        assert_eq!(facing_along(Vec2::new(-1.0, 0.1)), Some(Direction::West));
        assert_eq!(facing_along(Vec2::new(0.1, -1.0)), Some(Direction::North));
        assert_eq!(
            facing_along(Vec2::new(-0.7, -0.7)),
            Some(Direction::NorthWest)
        );
        assert_eq!(facing_along(Vec2::new(0.01, 0.02)), None);
    }

    /// A building rises from its foot: more of it shows every moment, it grows to a little over
    /// its size and settles, and it stands whole, at its size and clean, at the end.
    #[test]
    fn a_building_rises_from_its_foot_to_its_full_size() {
        let mut shown = 0.0;
        let mut largest: f32 = 0.0;
        for i in 0..=100 {
            let shape = rise_shape(i as f32 / 100.0);
            assert!(shape.reveal >= shown);
            shown = shape.reveal;
            largest = largest.max(shape.scale);
            assert!(shape.scale > 0.85 && shape.scale < 1.05);
        }
        assert!(rise_shape(0.0).reveal < 0.01);
        assert!(largest > 1.0);
        assert_eq!(
            rise_shape(1.0),
            RiseShape {
                reveal: 1.0,
                scale: 1.0,
                dust: 0.0
            }
        );
        assert!(
            rise_seconds(factory())
                > rise_seconds(Footprint {
                    width: 1,
                    height: 1,
                    ..factory()
                })
        );
    }

    /// The hall drawn behind an open door covers the doorway and stays inside the sprite.
    #[test]
    fn the_hall_fills_the_doorway() {
        let door = door_art("war_factory").expect("door");
        let tile = TileSize {
            width: 64.0,
            height: 32.0,
        };
        let (offset, size) = hall_place(door, factory(), tile);
        let sprite = crate::hd_art::building_size(3, 3);
        assert!(offset.x.abs() + size.x / 2.0 < sprite.x / 2.0);
        assert!(offset.y.abs() + size.y / 2.0 < sprite.y / 2.0);
        assert!(size.y > door.height && size.x >= door.width * 32.0);
    }

    /// Every point round a footprint's edge lies on it (pushed out by `out`).
    #[test]
    fn dust_rises_round_the_foundation() {
        let fp = factory();
        for i in 0..40 {
            let p = edge_point(fp, i as f32 / 40.0, 0.0);
            let on_x = p.x == 10.0 || p.x == 13.0;
            let on_y = p.y == 20.0 || p.y == 23.0;
            assert!(on_x || on_y, "{p}");
            assert!(
                (10.0..=13.0).contains(&p.x) && (20.0..=23.0).contains(&p.y),
                "{p}"
            );
        }
    }
}
