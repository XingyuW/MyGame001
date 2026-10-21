//! The minimap (PLAN.md section 7, M5; docs/DECISIONS.md D36): the whole map in the projection
//! of the main view, shrunk into an image at the top of the sidebar, with shroud and fog, the
//! human player's units and buildings, the enemies the human player is shown, and the rectangle
//! of the map the camera shows. A left click or drag on it centres the view there.
//!
//! The image is drawn from what the human player may know, as the main view is: the terrain as
//! the map file defines it ([`crate::map::MapTerrain`], like the tiles), black where the cell was
//! never seen, darkened where it is not seen now, and units and buildings of other players only
//! while `crate::units::unit_shown` and `crate::buildings::building_shown` show them. It never
//! reads the simulation's current terrain or the position of an enemy the player does not see.
//!
//! Since RA2 the minimap is the player's radar: it is drawn only while the human player has radar
//! (`sim::PlayerState::radar`: a working radar building, enough power, and no power outage from
//! a spy). Otherwise the map's diamond is blank, a "RADAR OFFLINE" panel says why
//! ([`offline_text`], shown by the sidebar), the camera's rectangle is hidden, and a click on
//! it moves nothing. A world without a rules file always has radar.
//!
//! Layout: continuous cell coordinates (u, v) of a W × H map map to the normalised image point
//! `((u − v + H) / (W + H), (u + v) / (W + H))`, the main view's projection (equation 7) scaled
//! so that the map's bounding rectangle fills the image. So the camera's view, an axis-aligned
//! rectangle in the world, is an axis-aligned rectangle on the minimap. The image has at least two
//! pixels per unit of `u − v` (M7, resolving M5 review 1 P2-2: with one, the cells with odd
//! `u + v` of a map with W + H > 128 had no pixel), so every cell of every map size the loader
//! accepts is drawn by at least one pixel.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::ui::RelativeCursorPosition;
use bevy::window::PrimaryWindow;
use sim::{Building, Cell, CellPos, Grid, Mobile, Owner, Vision};

use crate::fog::{Sight, human_vision, sight_of};
use crate::map::{MapTerrain, MapView};
use crate::projection::TileSize;
use crate::sidebar::SIDEBAR_WIDTH;
use crate::sim_host::SimHost;
use crate::units::HUMAN;

/// Width the minimap image aims for, in pixels.
pub const MINIMAP_TARGET_WIDTH: u32 = 256;

/// Size of the minimap on screen, in logical pixels (the image's aspect ratio is 2:1).
pub const MINIMAP_SIZE: Vec2 = Vec2::new(224.0, 112.0);

/// The colour of the map's diamond while the radar is offline: a dark screen, not the black of
/// the shroud.
pub const OFFLINE_COLOUR: [u8; 4] = [14, 20, 24, 255];

/// A public coordinate notice, independent of terrain visibility and normal contacts.
/// UI assumption: reuse the existing nine-real-second notice convention, including while
/// paused/offline. A hollow amber diamond marks only the supplied origin; it grants no sight.
#[derive(Resource, Default)]
pub(crate) struct PublicBuildingNotices {
    entries: Vec<PublicBuildingNotice>,
    revision: u64,
}

struct PublicBuildingNotice {
    building: sim::SimId,
    origin: CellPos,
    left: f32,
}

impl PublicBuildingNotices {
    /// Called only on the exact event batch returned by SimHost's scheduled tick.
    /// Never look up the building: an unseen death/move must not update this public notice.
    pub(crate) fn receive(&mut self, events: &[sim::SimEvent]) {
        for event in events {
            let sim::SimEvent::BuildingRevealed {
                building, origin, ..
            } = event
            else {
                continue;
            };
            self.entries.retain(|notice| notice.building != *building);
            if self.entries.len() >= crate::notices::MAX_NOTICES {
                self.entries.remove(0);
            }
            self.entries.push(PublicBuildingNotice {
                building: *building,
                origin: *origin,
                left: crate::notices::NOTICE_SECONDS,
            });
            self.revision = self.revision.saturating_add(1);
        }
    }
}

/// Synchronous host replacement reset: buffered old SimEvents cannot repopulate notices,
/// because this consumer uses run_tick's batch rather than a later MessageReader.
pub(crate) fn clear_public_building_notices(world: &mut World) {
    if let Some(mut notices) = world.get_resource_mut::<PublicBuildingNotices>() {
        notices.entries.clear();
        notices.revision = notices.revision.saturating_add(1);
    }
}

pub(crate) fn age_public_building_notices(
    time: Res<Time<Real>>,
    mut notices: ResMut<PublicBuildingNotices>,
) {
    let count = notices.entries.len();
    for notice in &mut notices.entries {
        notice.left -= time.delta_secs();
    }
    notices.entries.retain(|notice| notice.left > 0.0);
    if notices.entries.len() != count {
        notices.revision = notices.revision.saturating_add(1);
    }
}

/// Original raster primitive over the existing projected minimap image. The centre stays
/// intact, preserving an ordinary owned/known contact at the supplied coordinate.
fn draw_public_building_notices(
    layout: &MinimapLayout,
    notices: &PublicBuildingNotices,
    contacts: &[(CellPos, Marker, sim::PlayerId)],
    pixels: &mut [u8],
) {
    let (width, height) = layout.size();
    for notice in &notices.entries {
        let cell = notice.origin;
        if cell.x < 0
            || cell.y < 0
            || cell.x as u32 >= layout.map_width
            || cell.y as u32 >= layout.map_height
        {
            continue;
        }
        let centre = layout.normalised(cell.x as f32 + 0.5, cell.y as f32 + 0.5);
        let (cx, cy) = (
            (centre.x * width as f32) as i64,
            (centre.y * (width as f32 / 2.0)) as i64,
        );
        for dy in -3_i64..=3 {
            for dx in -3_i64..=3 {
                if dx.abs() + dy.abs() != 3 {
                    continue;
                }
                let (x, y) = (cx + dx, cy + dy);
                if x < 0
                    || y < 0
                    || x >= i64::from(width)
                    || y >= i64::from(height)
                    || layout.pixel_cell(x as u32, y as u32).is_none()
                {
                    continue;
                }
                // Ordinary known/owned contacts keep every pixel of their cell.
                if layout
                    .pixel_cell(x as u32, y as u32)
                    .is_some_and(|cell| contacts.iter().any(|(contact, _, _)| *contact == cell))
                {
                    continue;
                }
                let offset = (y as usize * width as usize + x as usize) * 4;
                if let Some(pixel) = pixels.get_mut(offset..offset + 4) {
                    pixel.copy_from_slice(&[255, 232, 120, 255]);
                }
            }
        }
    }
}

pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MinimapState>()
            // After the exclusive loading of the skirmish catalogue, so that no Startup system runs
            // unordered beside it. A match replaces the image with one of its map's size
            // ([`size_minimap`]).
            .add_systems(
                Startup,
                create_minimap.after(crate::skirmish::load_catalogue),
            )
            .add_systems(
                Update,
                (
                    click_minimap
                        .after(crate::camera::zoom_camera)
                        .before(crate::camera::clamp_camera),
                    (
                        age_public_building_notices,
                        draw_minimap,
                        place_camera_frame,
                    )
                        .chain()
                        .after(crate::camera::clamp_camera),
                ),
            );
    }
}

/// Size and scale of the minimap image of a `map_width` × `map_height` map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MinimapLayout {
    pub map_width: u32,
    pub map_height: u32,
    /// Image pixels per unit of `u − v`.
    pub scale: u32,
}

impl MinimapLayout {
    pub fn new(map_width: u32, map_height: u32) -> Self {
        let sum = map_width.saturating_add(map_height).max(1);
        Self {
            map_width,
            map_height,
            // At least two, so that every cell's diamond (2 × scale by scale pixels) holds a
            // pixel centre (M5 review 1 P2-2).
            scale: (MINIMAP_TARGET_WIDTH / sum).max(2),
        }
    }

    fn sum(&self) -> f32 {
        self.map_width.saturating_add(self.map_height).max(1) as f32
    }

    /// Width and height of the image in pixels.
    pub fn size(&self) -> (u32, u32) {
        let width = self
            .map_width
            .saturating_add(self.map_height)
            .max(1)
            .saturating_mul(self.scale);
        (width, width.div_ceil(2).max(1))
    }

    /// The normalised image point (0 to 1 on each axis) of continuous cell coordinates.
    pub fn normalised(&self, u: f32, v: f32) -> Vec2 {
        Vec2::new(
            (u - v + self.map_height as f32) / self.sum(),
            (u + v) / self.sum(),
        )
    }

    /// Continuous cell coordinates of a normalised image point.
    pub fn cell_coordinates(&self, point: Vec2) -> Vec2 {
        let difference = point.x * self.sum() - self.map_height as f32;
        let total = point.y * self.sum();
        Vec2::new((total + difference) / 2.0, (total - difference) / 2.0)
    }

    /// The cell under the centre of image pixel (`x`, `y`), if it lies on the map.
    pub fn pixel_cell(&self, x: u32, y: u32) -> Option<CellPos> {
        let (width, _) = self.size();
        let height = width as f32 / 2.0;
        let point = Vec2::new((x as f32 + 0.5) / width as f32, (y as f32 + 0.5) / height);
        let cell = self.cell_coordinates(point).floor();
        let (u, v) = (cell.x as i64, cell.y as i64);
        (u >= 0 && v >= 0 && u < i64::from(self.map_width) && v < i64::from(self.map_height))
            .then(|| CellPos::new(u as i32, v as i32))
    }

    /// The normalised image point of a world point (equation 7 inverted).
    pub fn point_of_world(&self, world: Vec2, tile: TileSize) -> Vec2 {
        let difference = world.x / (tile.width / 2.0);
        let total = -world.y / (tile.height / 2.0);
        self.normalised((total + difference) / 2.0, (total - difference) / 2.0)
    }

    /// The world point of a normalised image point.
    pub fn world_of_point(&self, point: Vec2, tile: TileSize) -> Vec2 {
        let difference = point.x * self.sum() - self.map_height as f32;
        let total = point.y * self.sum();
        Vec2::new(difference * tile.width / 2.0, -total * tile.height / 2.0)
    }
}

/// What a minimap cell shows besides terrain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marker {
    OwnUnit,
    OwnBuilding,
    EnemyUnit,
    EnemyBuilding,
}

/// The colour of a terrain cell as the human player sees it.
pub fn terrain_colour(cell: &Cell, sight: Sight) -> [u8; 4] {
    let base: [u8; 3] = if !cell.passable {
        [70, 80, 95]
    } else if cell.ore > 0 {
        [200, 170, 40]
    } else if cell.buildable {
        [60, 120, 50]
    } else {
        [170, 150, 100]
    };
    shade(base, sight)
}

/// The colour of a cell with a look (`battlefield::looks`): snow, desert, city and sea read as
/// such. Look 0 keeps the colour of the cell's terrain.
pub fn look_colour(cell: &Cell, look: u8, sight: Sight) -> [u8; 4] {
    use crate::battlefield::looks::{Feature, Ground, decode};
    let Some((ground, feature)) = decode(look).filter(|(g, _)| *g != Ground::Auto) else {
        return terrain_colour(cell, sight);
    };
    let base: [u8; 3] = match (feature, ground) {
        (Feature::Ore, _) => [200, 170, 40],
        (Feature::Gems, _) => [70, 196, 190],
        (Feature::Lava, _) => [214, 92, 30],
        (Feature::Cliff, Ground::Snow | Ground::Ice) => [150, 158, 170],
        (Feature::Cliff, Ground::Sand) => [150, 98, 64],
        (Feature::Cliff, Ground::Ash) => [58, 54, 54],
        (Feature::Cliff, _) => [92, 90, 82],
        (Feature::Forest, Ground::Snow | Ground::Ice) => [64, 92, 84],
        (Feature::Forest, Ground::Sand) => [96, 128, 58],
        (Feature::Forest, Ground::Marsh) => [38, 66, 44],
        (Feature::Forest, _) => [34, 76, 36],
        (Feature::House, _) => [150, 104, 84],
        (Feature::City, _) => [118, 118, 126],
        (Feature::Tower, _) => [170, 172, 184],
        (Feature::Road, _) => [84, 84, 86],
        (Feature::Bridge, _) => [128, 112, 92],
        (
            Feature::OilDerrick
            | Feature::Hospital
            | Feature::Airport
            | Feature::TechOutpost
            | Feature::BridgeHut,
            _,
        ) => [196, 188, 156],
        (Feature::None, Ground::Water) => [38, 78, 128],
        (Feature::None, Ground::Dirt) => [122, 100, 70],
        (Feature::None, Ground::Sand) => [196, 176, 124],
        (Feature::None, Ground::Snow) => [222, 228, 236],
        (Feature::None, Ground::Pavement) => [148, 148, 150],
        (Feature::None, Ground::Ice) => [168, 198, 216],
        (Feature::None, Ground::Crops) => [186, 164, 84],
        (Feature::None, Ground::Marsh) => [66, 86, 60],
        (Feature::None, Ground::Ash) => [78, 74, 72],
        (Feature::None, _) => [60, 120, 50],
    };
    shade(base, sight)
}

fn shade(base: [u8; 3], sight: Sight) -> [u8; 4] {
    match sight {
        Sight::Visible => [base[0], base[1], base[2], 255],
        Sight::Fog => {
            let dim = |channel: u8| (u16::from(channel) * 2 / 5) as u8;
            [dim(base[0]), dim(base[1]), dim(base[2]), 255]
        }
        Sight::Shroud => [0, 0, 0, 255],
    }
}

pub fn marker_colour(marker: Marker) -> [u8; 4] {
    match marker {
        Marker::OwnUnit => [110, 200, 255, 255],
        Marker::OwnBuilding => [40, 110, 230, 255],
        Marker::EnemyUnit => [255, 90, 70, 255],
        Marker::EnemyBuilding => [200, 40, 30, 255],
    }
}

/// The colour of every cell (row-major): terrain by `vision` (all shroud without one), then the
/// markers, which only name cells of units and buildings the human player is shown.
#[cfg(test)]
pub fn cell_colours(
    terrain: &Grid,
    vision: Option<&Vision>,
    markers: &[(CellPos, Marker)],
) -> Vec<[u8; 4]> {
    cell_colours_with_looks(terrain, &[], vision, markers)
}

/// [`cell_colours`] with the map's looks (row-major; cells beyond `looks` use their terrain).
pub fn cell_colours_with_looks(
    terrain: &Grid,
    looks: &[u8],
    vision: Option<&Vision>,
    markers: &[(CellPos, Marker)],
) -> Vec<[u8; 4]> {
    let (width, height) = (terrain.width(), terrain.height());
    let mut colours = Vec::with_capacity(width as usize * height as usize);
    for y in 0..height {
        for x in 0..width {
            let cell = CellPos::new(x as i32, y as i32);
            let sight = vision.map_or(Sight::Shroud, |vision| sight_of(vision, cell));
            let look = looks.get((y * width + x) as usize).copied().unwrap_or(0);
            let colour = terrain
                .cell(cell)
                .map_or([0, 0, 0, 255], |terrain| look_colour(terrain, look, sight));
            colours.push(colour);
        }
    }
    for &(cell, marker) in markers {
        if let (Ok(x), Ok(y)) = (u32::try_from(cell.x), u32::try_from(cell.y))
            && x < width
            && y < height
            && let Some(slot) = colours.get_mut((y * width + x) as usize)
        {
            *slot = marker_colour(marker);
        }
    }
    colours
}

/// The RGBA pixels of the minimap image: each pixel shows its cell's colour, transparent off the
/// map.
pub fn render(layout: &MinimapLayout, colours: &[[u8; 4]]) -> Vec<u8> {
    let (width, height) = layout.size();
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        for x in 0..width {
            let colour = layout
                .pixel_cell(x, y)
                .and_then(|cell| {
                    colours.get((cell.y as u32 * layout.map_width + cell.x as u32) as usize)
                })
                .copied()
                .unwrap_or([0, 0, 0, 0]);
            pixels.extend(colour);
        }
    }
    pixels
}

/// The markers of the units and buildings the human player is shown.
#[cfg(test)]
pub fn markers(host: &SimHost) -> Vec<(CellPos, Marker)> {
    owned_markers(host)
        .into_iter()
        .map(|(cell, marker, _)| (cell, marker))
        .collect()
}

/// [`markers`] with the owner of each, for drawing every player in its own team colour.
pub fn owned_markers(host: &SimHost) -> Vec<(CellPos, Marker, sim::PlayerId)> {
    let Some(players) = host.players() else {
        return Vec::new();
    };
    let mut markers = Vec::new();
    if let Some(mut units) = host.world().try_query::<(&sim::SimId, &Owner, &Mobile)>() {
        for (id, owner, mobile) in units.iter(host.world()) {
            if crate::units::unit_shown(owner.0, mobile, players)
                && !host
                    .world()
                    .get_resource::<sim::SpecialState>()
                    .is_some_and(|s| s.hidden.contains(&(HUMAN, *id)))
            {
                let marker = if owner.0 == HUMAN {
                    Marker::OwnUnit
                } else {
                    Marker::EnemyUnit
                };
                markers.push((mobile.cell, marker, owner.0));
            }
        }
    }
    if let Some(mut buildings) = host.world().try_query::<(&Owner, &Building)>() {
        for (owner, building) in buildings.iter(host.world()) {
            if crate::buildings::building_shown(owner.0, building.footprint, players) {
                let marker = if owner.0 == HUMAN {
                    Marker::OwnBuilding
                } else {
                    Marker::EnemyBuilding
                };
                markers.extend(
                    building
                        .footprint
                        .cells()
                        .map(|cell| (cell, marker, owner.0)),
                );
            }
        }
    }
    // Units are drawn over buildings (a unit never stands in a building's cell, I6).
    markers.sort_by_key(|(_, marker, _)| matches!(marker, Marker::OwnUnit | Marker::EnemyUnit));
    markers
}

/// The normalised rectangle of the minimap that `view` (a world rectangle) covers, clamped to
/// the minimap.
pub fn frame_rect(view: Rect, layout: &MinimapLayout, tile: TileSize) -> Rect {
    let a = layout.point_of_world(Vec2::new(view.min.x, view.max.y), tile);
    let b = layout.point_of_world(Vec2::new(view.max.x, view.min.y), tile);
    let unit = Rect::new(0.0, 0.0, 1.0, 1.0);
    Rect::from_corners(a, b).intersect(unit)
}

/// The world rectangle of the map the camera shows beside the sidebar: a view of `viewport`
/// logical pixels at projection `scale` centred on `centre`, less the `inset` pixels on its
/// right that the sidebar covers.
pub fn visible_view(centre: Vec2, viewport: Vec2, scale: f32, inset: f32) -> Rect {
    let half = viewport * scale / 2.0;
    Rect::new(
        centre.x - half.x,
        centre.y - half.y,
        centre.x + half.x - inset.min(viewport.x) * scale,
        centre.y + half.y,
    )
}

/// Whether the human player of `host`'s match has radar (RA2, `sim::PlayerState::radar`), so
/// that the minimap is drawn; also without a match.
pub fn radar_online(host: &SimHost) -> bool {
    host.players()
        .and_then(|players| players.get(HUMAN))
        .is_none_or(sim::PlayerState::radar)
}

/// What the panel over the blank minimap says while the human player has no radar, or `None`
/// while it has: that a radar building is missing, or, with one, that the base is short of
/// power (which a spy in a power plant also causes).
pub fn offline_text(host: &SimHost) -> Option<&'static str> {
    if radar_online(host) {
        return None;
    }
    let definitions = host.definitions()?;
    let owns_radar =
        host.world()
            .try_query::<(&Owner, &Building)>()
            .is_some_and(|mut buildings| {
                buildings.iter(host.world()).any(|(owner, building)| {
                    owner.0 == HUMAN
                        && definitions
                            .metadata(sim::ProductionItem::Building(building.kind))
                            .is_some_and(|entry| entry.radar)
                })
            });
    Some(if owns_radar {
        "RADAR OFFLINE\nlow power"
    } else {
        "RADAR OFFLINE\nno radar building"
    })
}

/// The image of a minimap without radar: the map's diamond in [`OFFLINE_COLOUR`].
pub fn offline_pixels(layout: &MinimapLayout) -> Vec<u8> {
    let cells = (layout.map_width as usize).saturating_mul(layout.map_height as usize);
    render(layout, &vec![OFFLINE_COLOUR; cells])
}

/// The pixels of a minimap image that show something of the map: opaque, not black (shroud)
/// and not of [`OFFLINE_COLOUR`]. None on the screen without radar; some on every drawn map on
/// which the player sees or has explored anything.
pub fn map_pixels(pixels: &[u8]) -> usize {
    pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[3] > 0 && pixel[..3] != [0, 0, 0] && **pixel != OFFLINE_COLOUR)
        .count()
}

// --- Systems --------------------------------------------------------------------------------

/// The minimap image and its layout.
#[derive(Resource, Debug)]
pub struct Minimap {
    pub image: Handle<Image>,
    pub layout: MinimapLayout,
}

/// The minimap node in the sidebar.
#[derive(Component, Debug)]
pub struct MinimapNode;

/// The camera's rectangle on the minimap.
#[derive(Component, Debug)]
pub struct MinimapFrame;

/// When the minimap was drawn; read by the smoke test.
#[derive(Resource, Debug, Default)]
pub struct MinimapState {
    /// Public-notice changes can require redrawing while the simulation is paused.
    notice_revision: u64,
    /// The simulation tick of the last drawing.
    pub drawn_tick: Option<u64>,
    /// Drawings so far.
    pub drawings: u64,
    /// Pixels on the map in the last drawing that are not black.
    pub lit_pixels: usize,
    /// Whether the last drawing was that of a minimap without radar (RA2).
    pub offline: bool,
    /// Pixels of the last drawing that show something of the map: on the map's diamond and
    /// neither black (shroud) nor of [`OFFLINE_COLOUR`]. Counted on the image as it was drawn,
    /// so that the smoke test can tell a drawn map from the screen without radar by what is
    /// on it, not by the flag above (advisory review 1, tests P1-1).
    pub map_pixels: usize,
}

pub(crate) fn create_minimap(
    mut commands: Commands,
    map: Option<Res<MapView>>,
    mut images: ResMut<Assets<Image>>,
) {
    let layout = map.map_or(MinimapLayout::new(1, 1), |map| {
        MinimapLayout::new(map.width, map.height)
    });
    commands.insert_resource(Minimap {
        image: images.add(blank_image(layout)),
        layout,
    });
}

/// A black, transparent image of the minimap's size.
fn blank_image(layout: MinimapLayout) -> Image {
    let (width, height) = layout.size();
    Image::new_fill(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// Gives the minimap a new image for a `width` × `height` map (a new match, M6), shown by the
/// sidebar's minimap node, and forgets the drawings of the previous match.
pub(crate) fn size_minimap(world: &mut World, width: u32, height: u32) {
    let layout = MinimapLayout::new(width, height);
    let image = world
        .resource_mut::<Assets<Image>>()
        .add(blank_image(layout));
    let nodes: Vec<Entity> = world
        .query_filtered::<Entity, With<MinimapNode>>()
        .iter(world)
        .collect();
    for node in nodes {
        world.entity_mut(node).insert(ImageNode::new(image.clone()));
    }
    world.insert_resource(Minimap { image, layout });
    world.insert_resource(MinimapState::default());
}

/// Redraws the minimap after every simulation tick.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_minimap(
    teams: Option<Res<crate::presentation::TeamColors>>,
    notices: Option<Res<PublicBuildingNotices>>,
    host: Res<SimHost>,
    terrain: Option<Res<MapTerrain>>,
    looks: Option<Res<crate::map::MapLooks>>,
    minimap: Option<Res<Minimap>>,
    mut state: ResMut<MinimapState>,
    mut images: ResMut<Assets<Image>>,
) {
    let (Some(terrain), Some(minimap)) = (terrain, minimap) else {
        return;
    };
    let tick = host.tick();
    let notice_revision = notices.as_deref().map_or(0, |notices| notices.revision);
    if state.drawn_tick == Some(tick) && state.notice_revision == notice_revision {
        return;
    }
    // Without radar nothing of the map is drawn (RA2).
    let online = radar_online(&host);
    let pixels = if online {
        let owned = owned_markers(&host);
        let marked: Vec<_> = owned
            .iter()
            .map(|&(cell, marker, _)| (cell, marker))
            .collect();
        let mut colours = cell_colours_with_looks(
            &terrain.0,
            looks.as_deref().map_or(&[], |l| l.0.as_slice()),
            human_vision(host.players()),
            &marked,
        );
        for &(cell, _, owner) in &owned {
            let rgb = teams
                .as_deref()
                .unwrap_or(&crate::presentation::TeamColors(0))
                .rgb_of(owner);
            if let Some(pixel) =
                colours.get_mut((cell.y as u32 * terrain.0.width() + cell.x as u32) as usize)
            {
                *pixel = [rgb[0], rgb[1], rgb[2], 255];
            }
        }
        let mut pixels = render(&minimap.layout, &colours);
        if let Some(notices) = notices.as_deref() {
            draw_public_building_notices(&minimap.layout, notices, &owned, &mut pixels);
        }
        pixels
    } else {
        offline_pixels(&minimap.layout)
    };
    state.offline = !online;
    state.lit_pixels = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[3] > 0 && pixel[..3] != [0, 0, 0])
        .count();
    state.map_pixels = map_pixels(&pixels);
    if let Some(mut image) = images.get_mut(&minimap.image)
        && image.data.as_ref() != Some(&pixels)
    {
        image.data = Some(pixels);
    }
    state.notice_revision = notice_revision;
    state.drawn_tick = Some(tick);
    state.drawings = state.drawings.saturating_add(1);
}

/// Places the camera's rectangle on the minimap.
pub(crate) fn place_camera_frame(
    map: Option<Res<MapView>>,
    minimap: Option<Res<Minimap>>,
    camera: Single<(&Camera, &Projection, &Transform), With<Camera2d>>,
    state: Res<MinimapState>,
    mut frames: Query<&mut Node, With<MinimapFrame>>,
) {
    let (Some(map), Some(minimap)) = (map, minimap) else {
        return;
    };
    // No rectangle on a minimap without radar.
    let display = if state.offline {
        Display::None
    } else {
        Display::Flex
    };
    for mut node in &mut frames {
        if node.display != display {
            node.display = display;
        }
    }
    let (camera, projection, transform) = *camera;
    let (Some(viewport), Projection::Orthographic(ortho)) =
        (camera.logical_viewport_size(), projection)
    else {
        return;
    };
    let view = visible_view(
        transform.translation.truncate(),
        viewport,
        ortho.scale,
        SIDEBAR_WIDTH,
    );
    let rect = frame_rect(view, &minimap.layout, map.tile);
    for mut node in &mut frames {
        let (left, top) = (
            Val::Percent(rect.min.x * 100.0),
            Val::Percent(rect.min.y * 100.0),
        );
        let (width, height) = (
            Val::Percent(rect.width() * 100.0),
            Val::Percent(rect.height() * 100.0),
        );
        if node.left != left || node.top != top || node.width != width || node.height != height {
            node.left = left;
            node.top = top;
            node.width = width;
            node.height = height;
        }
    }
}

/// A left click or drag that starts on the minimap centres the visible part of the view on the
/// point under the cursor. Not while the radar is offline (RA2).
#[allow(clippy::too_many_arguments)]
fn click_minimap(
    buttons: Res<ButtonInput<MouseButton>>,
    state: Res<MinimapState>,
    map: Option<Res<MapView>>,
    minimap: Option<Res<Minimap>>,
    nodes: Query<&RelativeCursorPosition, With<MinimapNode>>,
    camera: Single<(&Projection, &mut Transform), With<Camera2d>>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut dragging: Local<bool>,
) {
    let (Some(map), Some(minimap)) = (map, minimap) else {
        return;
    };
    let over = nodes.iter().find(|cursor| cursor.cursor_over());
    if buttons.just_pressed(MouseButton::Left) {
        *dragging = over.is_some();
    }
    if !buttons.pressed(MouseButton::Left) {
        *dragging = false;
    }
    if !*dragging || window.cursor_position().is_none() || state.offline {
        return;
    }
    // While dragging, the cursor may leave the minimap; the point is then clamped to its edge.
    let Some(point) = nodes
        .iter()
        .find_map(|cursor| cursor.normalized)
        .map(|point| point.clamp(Vec2::splat(-0.5), Vec2::splat(0.5)))
    else {
        return;
    };
    let (projection, mut transform) = camera.into_inner();
    let Projection::Orthographic(ortho) = projection else {
        return;
    };
    let world = minimap
        .layout
        .world_of_point(point + Vec2::splat(0.5), map.tile);
    // The visible part of the view lies left of the sidebar, so its centre is shifted.
    let centre = world + Vec2::new(SIDEBAR_WIDTH * ortho.scale / 2.0, 0.0);
    transform.translation.x = centre.x;
    transform.translation.y = centre.y;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projection::{map_bounds, to_world};

    const TILE: TileSize = TileSize {
        width: 64.0,
        height: 32.0,
    };

    #[test]
    fn the_layout_fills_the_image_with_the_maps_bounding_rectangle() {
        for (width, height) in [(1, 1), (1, 37), (37, 1), (64, 64), (256, 256)] {
            let layout = MinimapLayout::new(width, height);
            let (pixels_wide, pixels_high) = layout.size();
            assert!(
                pixels_wide >= 1 && pixels_wide <= 2 * MINIMAP_TARGET_WIDTH.max(width + height)
            );
            assert_eq!(pixels_high, pixels_wide.div_ceil(2));
            // The four corners of the map's diamond lie on the edges of the image.
            let (w, h) = (width as f32, height as f32);
            assert_eq!(layout.normalised(0.0, 0.0).y, 0.0);
            assert_eq!(layout.normalised(w, h).y, 1.0);
            assert_eq!(layout.normalised(0.0, h).x, 0.0);
            assert_eq!(layout.normalised(w, 0.0).x, 1.0);
            // The map's bounding rectangle in the world is the whole image.
            let bounds = map_bounds(width, height, TILE);
            let frame = frame_rect(bounds, &layout, TILE);
            assert_eq!(frame, Rect::new(0.0, 0.0, 1.0, 1.0), "{width}x{height}");
            // World and image points convert both ways.
            let world = to_world(0.3 * w, 0.6 * h, TILE);
            let back = layout.world_of_point(layout.point_of_world(world, TILE), TILE);
            assert!(
                back.distance(world) < 1e-2,
                "{width}x{height}: {back} {world}"
            );
        }
        // The demo map: 256 x 128 pixels, two per unit of u - v.
        assert_eq!(MinimapLayout::new(64, 64).size(), (256, 128));
    }

    #[test]
    fn every_cell_of_every_accepted_map_size_is_drawn_by_a_pixel() {
        // M5 review 1 P2-2: at one pixel per unit of u - v (W + H > 128), the cells with odd
        // u + v had no pixel, so units on them were never marked. The sizes from the smallest to
        // the largest map the loader accepts (sim::MAX_MAP_SIDE), across W + H = 128 and 129.
        let largest = sim::MAX_MAP_SIDE;
        for (width, height) in [
            (1, 1),
            (1, largest),
            (largest, 1),
            (64, 64),
            (65, 64),
            (64, 65),
            (100, 100),
            (127, 2),
            (128, 128),
            (200, 56),
            (largest, largest),
        ] {
            let layout = MinimapLayout::new(width, height);
            assert!(layout.scale >= 2, "{width}x{height}");
            let (pixels_wide, pixels_high) = layout.size();
            let mut seen = vec![false; (width * height) as usize];
            for y in 0..pixels_high {
                for x in 0..pixels_wide {
                    if let Some(cell) = layout.pixel_cell(x, y) {
                        seen[(cell.y as u32 * width + cell.x as u32) as usize] = true;
                    }
                }
            }
            let missing = seen.iter().filter(|drawn| !**drawn).count();
            assert_eq!(
                missing, 0,
                "{width}x{height}: {missing} cells without a pixel"
            );
        }
        // The largest image: 1,024 x 512 pixels.
        assert_eq!(
            MinimapLayout::new(largest, largest).size(),
            (4 * largest, 2 * largest)
        );
    }

    #[test]
    fn every_pixel_shows_the_cell_under_it() {
        for (width, height) in [(1, 1), (3, 5), (64, 64)] {
            let layout = MinimapLayout::new(width, height);
            let (pixels_wide, pixels_high) = layout.size();
            let mut seen = std::collections::BTreeSet::new();
            for y in 0..pixels_high {
                for x in 0..pixels_wide {
                    if let Some(cell) = layout.pixel_cell(x, y) {
                        // The pixel's centre lies in that cell's diamond.
                        let centre = Vec2::new(
                            (x as f32 + 0.5) / pixels_wide as f32,
                            (y as f32 + 0.5) / (pixels_wide as f32 / 2.0),
                        );
                        let coordinates = layout.cell_coordinates(centre);
                        assert_eq!(coordinates.floor(), Vec2::new(cell.x as f32, cell.y as f32));
                        seen.insert(cell);
                    }
                }
            }
            // On a map small enough for several pixels per cell, every cell is drawn.
            if layout.scale >= 4 {
                assert_eq!(seen.len(), (width * height) as usize, "{width}x{height}");
            }
            // Corners of the image lie off the map.
            assert_eq!(layout.pixel_cell(0, 0), None);
            assert_eq!(layout.pixel_cell(pixels_wide - 1, pixels_high - 1), None);
        }
    }

    #[test]
    fn the_minimap_shows_shroud_fog_and_only_shown_units() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../sim/tests/fixtures/maps/open_16x16.tmx");
        let terrain = sim::load_map(path)
            .unwrap_or_else(|error| panic!("{error}"))
            .grid;
        assert_eq!(terrain.width(), 16);
        // Without a vision everything is shroud, also under a marker-free enemy.
        let colours = cell_colours(&terrain, None, &[]);
        assert!(colours.iter().all(|colour| *colour == [0, 0, 0, 255]));
        let cell = terrain.cell(CellPos::new(3, 3)).copied().expect("cell");
        assert_eq!(terrain_colour(&cell, Sight::Shroud), [0, 0, 0, 255]);
        let visible = terrain_colour(&cell, Sight::Visible);
        let fog = terrain_colour(&cell, Sight::Fog);
        assert_ne!(visible, fog);
        assert_ne!(fog, [0, 0, 0, 255]);
        // Markers are drawn on their cells, and only there.
        let colours = cell_colours(
            &terrain,
            None,
            &[
                (CellPos::new(2, 0), Marker::OwnUnit),
                (CellPos::new(-1, 0), Marker::EnemyUnit),
                (CellPos::new(16, 0), Marker::EnemyUnit),
            ],
        );
        assert_eq!(colours[2], marker_colour(Marker::OwnUnit));
        assert_eq!(colours.iter().filter(|c| **c != [0, 0, 0, 255]).count(), 1);
        let layout = MinimapLayout::new(16, 16);
        let pixels = render(&layout, &colours);
        assert_eq!(
            pixels.len(),
            (layout.size().0 * layout.size().1 * 4) as usize
        );
    }

    #[test]
    fn the_demo_match_shows_no_enemy_the_human_player_does_not_see() {
        let host = crate::sim_host::tests::demo_host();
        // Before the first tick nothing is visible: only the human player's own units and
        // buildings are marked.
        let before = markers(&host);
        assert!(!before.is_empty());
        assert!(
            before
                .iter()
                .all(|(_, marker)| matches!(marker, Marker::OwnUnit | Marker::OwnBuilding)),
            "{before:?}"
        );
        assert_eq!(
            before
                .iter()
                .filter(|(_, marker)| *marker == Marker::OwnBuilding)
                .count(),
            9,
            "the human player's 3x3 construction yard"
        );
    }

    #[test]
    fn an_enemy_is_marked_exactly_while_the_human_player_sees_it() {
        use crate::units::OPPONENT;
        // A rifleman of the human player sees 5 cells; one enemy crawler stands at the edge of
        // its sight, the other one cell beyond.
        let mut host = crate::sim_host::tests::fixture_host(|world| {
            for player in [HUMAN, OPPONENT] {
                world
                    .resource_mut::<sim::Players>()
                    .add(player, 0)
                    .expect("player");
            }
            sim::spawn_unit(world, HUMAN, "rifleman", CellPos::new(10, 16)).expect("own");
            sim::spawn_unit(world, OPPONENT, "crawler", CellPos::new(15, 16)).expect("near");
            sim::spawn_unit(world, OPPONENT, "crawler", CellPos::new(16, 16)).expect("far");
        });
        assert_eq!(markers(&host), [(CellPos::new(10, 16), Marker::OwnUnit)]);
        crate::sim_host::tests::run(&mut host, Vec::new());
        let mut marked = markers(&host);
        marked.sort_by_key(|(cell, _)| *cell);
        assert_eq!(
            marked,
            [
                (CellPos::new(10, 16), Marker::OwnUnit),
                (CellPos::new(15, 16), Marker::EnemyUnit)
            ]
        );
        // The unseen crawler's cell looks like any other cell beyond sight.
        let terrain = host.grid().expect("grid").clone();
        let colours = cell_colours(&terrain, human_vision(host.players()), &marked);
        assert_eq!(colours[16 * 32 + 16], colours[16 * 32 + 17]);
    }

    /// Spawns a building of type `id` for the human player on the first free ground of the
    /// demo map, row by row from (20, 30).
    fn build(world: &mut World, id: &str) {
        for y in 30..60 {
            for x in 20..60 {
                if sim::spawn_building(world, HUMAN, id, CellPos::new(x, y)).is_ok() {
                    return;
                }
            }
        }
        panic!("no ground for {id}");
    }

    #[test]
    fn the_minimap_is_drawn_only_while_the_human_player_has_radar() {
        // RA2. The demo match with the shipped data: the human player owns its Construction
        // Yard and no radar building, so the screen is blank and says why.
        let mut host = crate::sim_host::tests::demo_host();
        crate::sim_host::tests::run(&mut host, Vec::new());
        assert!(!radar_online(&host));
        assert_eq!(
            offline_text(&host),
            Some("RADAR OFFLINE\nno radar building")
        );
        // With a power plant and an Airforce Command the map is drawn.
        let mut host = crate::sim_host::tests::demo_host_with(|world| {
            build(world, "power_plant");
            build(world, "american_airforce_command");
        });
        crate::sim_host::tests::run(&mut host, Vec::new());
        assert!(radar_online(&host));
        assert_eq!(offline_text(&host), None);
        // Three Prism Towers more than the power plant feeds: low power, no radar.
        let mut host = crate::sim_host::tests::demo_host_with(|world| {
            build(world, "power_plant");
            build(world, "american_airforce_command");
            for _ in 0..3 {
                build(world, "prism_tower");
            }
        });
        crate::sim_host::tests::run(&mut host, Vec::new());
        let state = host.players().and_then(|players| players.get(HUMAN));
        assert!(state.is_some_and(sim::PlayerState::low_power));
        assert!(!radar_online(&host));
        assert_eq!(offline_text(&host), Some("RADAR OFFLINE\nlow power"));
        // Without a human player there is nothing to switch off.
        let empty = crate::sim_host::tests::fixture_host(|_| {});
        assert!(radar_online(&empty));
        assert_eq!(offline_text(&empty), None);
    }

    /// What [`draw_minimap`], run as the system it is, draws for `host`: the image, the
    /// layout, and the state it leaves.
    fn drawn(host: SimHost) -> (Vec<u8>, MinimapLayout, MinimapState) {
        use bevy::ecs::system::RunSystemOnce;
        let grid = host.grid().expect("grid").clone();
        let layout = MinimapLayout::new(grid.width(), grid.height());
        let mut world = World::new();
        let mut images = Assets::<Image>::default();
        let image = images.add(Image::default());
        world.insert_resource(images);
        world.insert_resource(MapTerrain(grid));
        world.insert_resource(Minimap {
            image: image.clone(),
            layout,
        });
        world.insert_resource(MinimapState::default());
        world.insert_resource(host);
        world
            .run_system_once(draw_minimap)
            .expect("the system runs");
        let data = world
            .resource::<Assets<Image>>()
            .get(&image)
            .and_then(|image| image.data.clone())
            .expect("the image was drawn");
        let state = world.remove_resource::<MinimapState>().expect("the state");
        (data, layout, state)
    }

    /// Regression (advisory review 1, tests P1-1): no test ran `draw_minimap`, and the smoke
    /// test compared the flag the drawing sets with the radar it was set from, which cannot
    /// differ. What is drawn is now looked at: the map with radar, the screen of one colour
    /// without, and the count the smoke test reads ([`MinimapState::map_pixels`]) tells them
    /// apart.
    #[test]
    fn the_drawing_is_the_map_with_radar_and_the_offline_screen_without() {
        // No radar building: the offline screen, pixel for pixel.
        let mut host = crate::sim_host::tests::demo_host();
        crate::sim_host::tests::run(&mut host, Vec::new());
        assert!(!radar_online(&host));
        let (data, layout, state) = drawn(host);
        assert_eq!(data, offline_pixels(&layout));
        assert!(state.offline && state.drawings == 1);
        assert_eq!(state.map_pixels, 0);
        assert!(state.lit_pixels > 0, "the screen is not black");
        // A power plant and an Airforce Command: the map, which is not that screen.
        let mut host = crate::sim_host::tests::demo_host_with(|world| {
            build(world, "power_plant");
            build(world, "american_airforce_command");
        });
        crate::sim_host::tests::run(&mut host, Vec::new());
        assert!(radar_online(&host));
        let (data, layout, state) = drawn(host);
        assert_ne!(data, offline_pixels(&layout));
        assert!(!state.offline);
        assert!(
            state.map_pixels > 0,
            "terrain and the player's own buildings"
        );
        assert_eq!(state.map_pixels, map_pixels(&data));
        // Low power: the offline screen again.
        let mut host = crate::sim_host::tests::demo_host_with(|world| {
            build(world, "power_plant");
            build(world, "american_airforce_command");
            for _ in 0..3 {
                build(world, "prism_tower");
            }
        });
        crate::sim_host::tests::run(&mut host, Vec::new());
        assert!(!radar_online(&host));
        let (data, layout, state) = drawn(host);
        assert_eq!(data, offline_pixels(&layout));
        assert!(state.offline);
        assert_eq!(state.map_pixels, 0);
        // The count tells the two apart on any image: the offline screen has none, and one
        // pixel of the map is enough.
        let mut screen = offline_pixels(&layout);
        assert_eq!(map_pixels(&screen), 0);
        let on_map = screen
            .as_chunks::<4>()
            .0
            .iter()
            .position(|pixel| pixel[3] > 0)
            .expect("the diamond");
        screen[on_map * 4..on_map * 4 + 4].copy_from_slice(&[40, 90, 40, 255]);
        assert_eq!(map_pixels(&screen), 1);
    }

    #[test]
    fn the_offline_screen_fills_the_maps_diamond_and_shows_nothing_of_it() {
        let layout = MinimapLayout::new(64, 64);
        let pixels = offline_pixels(&layout);
        let (width, height) = layout.size();
        assert_eq!(pixels.len(), (width * height * 4) as usize);
        let (mut lit, mut clear) = (0, 0);
        for pixel in pixels.as_chunks::<4>().0 {
            if pixel[3] == 0 {
                clear += 1;
            } else {
                assert_eq!(*pixel, OFFLINE_COLOUR);
                lit += 1;
            }
        }
        // As many pixels as the map drawn with radar covers, none of them black.
        let drawn = render(&layout, &vec![[9, 9, 9, 255]; 64 * 64]);
        let covered = drawn.as_chunks::<4>().0.iter().filter(|p| p[3] > 0).count();
        assert_eq!(lit, covered);
        assert!(lit > 0 && clear > 0);
        assert_ne!(OFFLINE_COLOUR[..3], [0, 0, 0]);
    }

    #[test]
    fn the_camera_frame_is_the_visible_part_of_the_view() {
        let view = visible_view(Vec2::new(100.0, 50.0), Vec2::new(1280.0, 720.0), 1.0, 240.0);
        assert_eq!(view, Rect::new(-540.0, -310.0, 500.0, 410.0));
        let zoomed = visible_view(Vec2::ZERO, Vec2::new(1280.0, 720.0), 2.0, 240.0);
        assert_eq!(zoomed.width(), 2.0 * (1280.0 - 240.0));
        // A view larger than the map is clamped to the minimap.
        let layout = MinimapLayout::new(4, 4);
        let huge = Rect::new(-1.0e6, -1.0e6, 1.0e6, 1.0e6);
        assert_eq!(
            frame_rect(huge, &layout, TILE),
            Rect::new(0.0, 0.0, 1.0, 1.0)
        );
    }
}

// UNCOMPILED / UNRUN / UNACCEPTED. Append this sibling module to game/src/minimap.rs.
// Stock definitions throughout. Independent open geometry is test setup, not an engine/source
// permission. The UI consumer still uses only production owned_markers -> draw_minimap.
#[cfg(test)]
pub(crate) mod ra9_behavior87_routes {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use sim::{Command, IssuedCommand, PlayerId, RejectReason, SimEvent, SimId};
    const P1: PlayerId = PlayerId(1);
    #[derive(Clone, Copy, Debug)]
    enum Mission {
        Asw,
        Dmisl,
        Hornet,
        V3,
        Plane,
    }
    impl Mission {
        fn section(self) -> &'static str {
            match self {
                Self::Asw => "ASW",
                Self::Dmisl => "DMISL",
                Self::Hornet => "HORNET",
                Self::V3 => "V3ROCKET",
                Self::Plane => "PDPLANE",
            }
        }
        fn name(self) -> &'static str {
            match self {
                Self::Asw => "osprey",
                Self::Dmisl => "dred_missile",
                Self::Hornet => "hornet",
                Self::V3 => "v3_rocket",
                Self::Plane => "paradrop_plane",
            }
        }
    }
    fn issue(player: PlayerId, command: Command) -> IssuedCommand {
        IssuedCommand { player, command }
    }
    fn tick(host: &mut SimHost, commands: Vec<IssuedCommand>) -> Vec<SimEvent> {
        crate::sim_host::tests::run_issued(host, commands)
    }
    fn rejected(events: &[SimEvent]) -> Vec<RejectReason> {
        events
            .iter()
            .filter_map(|e| {
                if let SimEvent::CommandRejected { reason, .. } = e {
                    Some(reason.clone())
                } else {
                    None
                }
            })
            .collect()
    }
    fn entity(host: &SimHost, id: SimId) -> bevy::ecs::entity::Entity {
        host.world()
            .try_query::<(Entity, &SimId)>()
            .unwrap()
            .iter(host.world())
            .find(|(_, found)| **found == id)
            .map(|(e, _)| e)
            .expect("real live actor")
    }
    fn mobile(host: &SimHost, id: SimId) -> sim::Mobile {
        *host
            .world()
            .get::<sim::Mobile>(entity(host, id))
            .expect("real mobile")
    }
    fn destination(host: &SimHost, id: SimId) -> Option<CellPos> {
        host.world()
            .get::<sim::Orders>(entity(host, id))
            .unwrap()
            .current
            .as_ref()
            .map(|o| o.destination)
    }
    fn unit(world: &mut World, p: PlayerId, name: &str, x: i32, y: i32) -> SimId {
        sim::spawn_unit(world, p, name, CellPos::new(x, y)).unwrap()
    }
    fn building(world: &mut World, p: PlayerId, name: &str, x: i32, y: i32) -> SimId {
        sim::spawn_building(world, p, name, CellPos::new(x, y)).unwrap()
    }
    fn stock_host(water_from: i32, setup: impl FnOnce(&mut World)) -> SimHost {
        crate::sim_host::tests::fixture_host(|world| {
            let d = sim::load_ruleset(
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data"),
            )
            .unwrap();
            let cells = (0..48)
                .flat_map(|y| {
                    (0..48).map(move |x| {
                        let water = x >= water_from && !(x <= 8 && y <= 5);
                        sim::Cell {
                            passable: !water,
                            water,
                            buildable: !water,
                            ore: 0,
                        }
                    })
                })
                .collect();
            world.insert_resource(sim::Grid::from_cells(48, 48, cells).unwrap());
            for (p, country) in [(HUMAN, "great_britain"), (P1, "russia")] {
                world.resource_mut::<sim::Players>().add(p, 10_000).unwrap();
                assert!(
                    world
                        .resource_mut::<sim::Players>()
                        .set_country(p, Some(d.rules().unwrap().country_id(country).unwrap()))
                );
            }
            world.insert_resource(d);
            setup(world);
        })
    }
    struct Launched {
        host: SimHost,
        id: SimId,
        parent: SimId,
        power: SimId,
        owner: PlayerId,
    }
    fn launch(kind: Mission, owner: PlayerId, human_observer: bool) -> Launched {
        launch_in(kind, owner, human_observer, false)
    }
    fn launch_in(kind: Mission, owner: PlayerId, human_observer: bool, running: bool) -> Launched {
        let other = if owner == HUMAN { P1 } else { HUMAN };
        let mut parent = None;
        let mut target = None;
        let mut power = None;
        let water_from = match kind {
            Mission::Asw => 0,
            Mission::Dmisl | Mission::Hornet => 24,
            _ => 48,
        };
        let setup = |w: &mut World| {
            power = Some(building(w, HUMAN, "nuclear_reactor", 1, 1));
            building(w, HUMAN, "airforce_command", 6, 1);
            let (producer, px, py, victim, tx, ty, spotter, sx, sy) = match kind {
                Mission::Asw => (
                    "destroyer",
                    10,
                    10,
                    "typhoon",
                    18,
                    10,
                    "amphibious_transport",
                    13,
                    13,
                ),
                Mission::Dmisl => ("dreadnought", 36, 10, "grizzly", 22, 10, "engineer", 19, 10),
                Mission::Hornet => (
                    "aircraft_carrier",
                    34,
                    10,
                    "grizzly",
                    22,
                    10,
                    "engineer",
                    19,
                    10,
                ),
                Mission::V3 => ("v3_launcher", 10, 10, "grizzly", 26, 10, "engineer", 23, 10),
                Mission::Plane => ("airport", 8, 8, "engineer", 20, 20, "engineer", 21, 20),
            };
            parent = Some(if matches!(kind, Mission::Plane) {
                building(w, owner, producer, px, py)
            } else {
                unit(w, owner, producer, px, py)
            });
            // The strategic target is our own scout's actually observed cell; no enemy lookup.
            target = Some(unit(
                w,
                if matches!(kind, Mission::Plane) {
                    owner
                } else {
                    other
                },
                victim,
                tx,
                ty,
            ));
            unit(w, owner, spotter, sx, sy);
            if human_observer && owner != HUMAN {
                let (x, y) = match kind {
                    Mission::Asw => (13, 14),
                    Mission::Dmisl => (34, 14),
                    Mission::Hornet => (32, 14),
                    Mission::V3 => (14, 14),
                    Mission::Plane => (20, 4),
                };
                unit(w, HUMAN, "amphibious_transport", x, y);
            }
        };
        let mut host = if running {
            c62_running_stock_host(water_from, setup)
        } else {
            stock_host(water_from, setup)
        };
        let parent = parent.unwrap();
        let target = target.unwrap();
        tick(&mut host, vec![]);
        let mut events;
        let id;
        if matches!(kind, Mission::Plane) {
            let k = host
                .definitions()
                .unwrap()
                .building_kind("airport")
                .unwrap();
            let cooldown = host
                .definitions()
                .unwrap()
                .metadata(sim::ProductionItem::Building(k))
                .unwrap()
                .cooldown;
            assert!(cooldown > 0 && cooldown <= 10_000);
            let mut full = false;
            for _ in 0..=cooldown {
                full = host
                    .world()
                    .resource::<sim::SpecialState>()
                    .charge
                    .get(&parent)
                    == Some(&cooldown);
                if full {
                    break;
                }
                tick(&mut host, vec![]);
            }
            assert!(full, "natural charge after first-owner announcement");
            events = tick(
                &mut host,
                vec![issue(
                    owner,
                    Command::Superweapon {
                        building: parent,
                        target: CellPos::new(20, 20),
                        units: vec![],
                    },
                )],
            );
            assert!(rejected(&events).is_empty(), "{events:?}");
            assert!(
                events
                    .iter()
                    .any(|e| matches!(e,SimEvent::StrategicActivated{
                player,building,..}if *player==owner&&*building==parent))
            );
            let state = host.world().resource::<sim::SpecialState>();
            let flights: Vec<_> = state
                .strategic
                .flights
                .iter()
                .filter(|(_, f)| f.owner == owner)
                .map(|(&id, _)| id)
                .collect();
            assert_eq!(
                flights.len(),
                1,
                "created only by the real strategic command"
            );
            id = flights[0];
        } else {
            events = tick(
                &mut host,
                vec![issue(
                    owner,
                    Command::Attack {
                        units: vec![parent],
                        target,
                    },
                )],
            );
            assert!(
                rejected(&events).is_empty(),
                "{kind:?}: actual known launcher target: {events:?}"
            );
            let mut found = None;
            for _ in 0..80 {
                found = events.iter().find_map(|e| match e {
                    SimEvent::UnitLaunched {
                        player,
                        parent: p,
                        unit,
                    } if *player == owner && *p == parent => Some(*unit),
                    _ => None,
                });
                if found.is_some() {
                    break;
                }
                events = tick(&mut host, vec![]);
            }
            id = found.expect("actual UnitLaunched within bounded stock launch latency");
            assert_eq!(
                host.world()
                    .resource::<sim::SpecialState>()
                    .spawned
                    .get(&id),
                Some(&parent)
            );
        }
        let d = host.definitions().unwrap();
        assert_eq!(
            host.world()
                .get::<sim::Unit>(entity(&host, id))
                .unwrap()
                .kind,
            d.unit_kind(kind.name()).expect("stock mission name")
        );
        assert_eq!(
            host.world().get::<sim::Owner>(entity(&host, id)).unwrap().0,
            owner
        );
        let start = mobile(&host, id).cell;
        let mut separate = false;
        for _ in 0..64 {
            if mobile(&host, id).cell != start {
                separate = true;
                break;
            }
            tick(&mut host, vec![]);
        }
        assert!(
            separate,
            "real internal flight moved away from launch/parent pixel"
        );
        assert!(
            host.world()
                .get::<sim::Health>(entity(&host, id))
                .unwrap()
                .hit_points
                > 0
        );
        assert!(radar_online(&host));
        Launched {
            host,
            id,
            parent,
            power: power.unwrap(),
            owner,
        }
    }
    fn public_route(kind: Mission) {
        for observer in [false, true] {
            let Launched {
                mut host,
                id,
                parent,
                owner,
                ..
            } = launch(kind, P1, observer);
            let before = mobile(&host, id).sub_position();
            let order = destination(&host, id);
            let events = tick(
                &mut host,
                vec![issue(
                    owner,
                    Command::Move {
                        units: vec![id],
                        target: CellPos::new(40, 40),
                    },
                )],
            );
            assert_eq!(rejected(&events), vec![RejectReason::NotCommandable(id)]);
            assert_eq!(
                destination(&host, id),
                order,
                "refusal cannot replace the internal mission"
            );
            let known = host
                .players()
                .unwrap()
                .get(HUMAN)
                .unwrap()
                .vision()
                .sees_unit(&mobile(&host, id));
            assert_eq!(
                known,
                observer,
                "{}: stock early-flight observer geometry",
                kind.section()
            );
            let missing = SimId(u32::MAX);
            let events = tick(
                &mut host,
                vec![
                    issue(
                        HUMAN,
                        Command::Move {
                            units: vec![id],
                            target: CellPos::new(40, 40),
                        },
                    ),
                    issue(
                        HUMAN,
                        Command::Move {
                            units: vec![missing],
                            target: CellPos::new(40, 40),
                        },
                    ),
                ],
            );
            assert_eq!(
                rejected(&events),
                vec![
                    if known {
                        RejectReason::NotOwned(id)
                    } else {
                        RejectReason::UnknownUnit(id)
                    },
                    RejectReason::UnknownUnit(missing)
                ]
            );
            let mut progressed = mobile(&host, id).sub_position() != before;
            for _ in 0..32 {
                if progressed {
                    break;
                }
                tick(&mut host, vec![]);
                progressed = mobile(&host, id).sub_position() != before;
            }
            assert!(
                progressed,
                "public rejection does not stall the genuine mission"
            );
            assert_eq!(
                host.world()
                    .get::<sim::Owner>(entity(&host, parent))
                    .unwrap()
                    .0,
                owner
            );
            // Dark here means dark to HUMAN, not permission to send issuer-controlled flight
            // into issuer shroud. Its own Sight/internal mission can reveal its path.
        }
    }
    fn draw_world(host: SimHost) -> World {
        let grid = host.grid().unwrap().clone();
        let layout = MinimapLayout::new(grid.width(), grid.height());
        let mut world = World::new();
        let mut images = Assets::<Image>::default();
        let image = images.add(Image::default());
        world.insert_resource(images);
        world.insert_resource(MapTerrain(grid));
        world.insert_resource(Minimap { image, layout });
        world.insert_resource(MinimapState::default());
        world.insert_resource(host);
        world
    }
    fn draw(world: &mut World) {
        world.run_system_once(draw_minimap).unwrap();
        assert_eq!(
            world.resource::<MinimapState>().drawn_tick,
            Some(world.resource::<SimHost>().tick())
        );
    }
    fn pixels(world: &World) -> &[u8] {
        let minimap = world.resource::<Minimap>();
        world
            .resource::<Assets<Image>>()
            .get(&minimap.image)
            .unwrap()
            .data
            .as_ref()
            .unwrap()
    }
    fn has_marker(host: &SimHost, cell: CellPos, owner: PlayerId) -> bool {
        owned_markers(host).contains(&(
            cell,
            if owner == HUMAN {
                Marker::OwnUnit
            } else {
                Marker::EnemyUnit
            },
            owner,
        ))
    }
    fn assert_contact_pixels(world: &World, cell: CellPos, owner: PlayerId, present: bool) {
        let rgb = crate::presentation::TeamColors(0).rgb_of(owner);
        let colour = [rgb[0], rgb[1], rgb[2], 255];
        let layout = world.resource::<Minimap>().layout;
        let (width, height) = layout.size();
        let mut count = 0;
        for y in 0..height {
            for x in 0..width {
                if layout.pixel_cell(x, y) == Some(cell) {
                    count += 1;
                    let p = ((y * width + x) * 4) as usize;
                    assert_eq!(
                        &pixels(world)[p..p + 4] == colour.as_slice(),
                        present,
                        "actual projected subject pixel, not a synthetic marker"
                    );
                }
            }
        }
        assert!(
            count > 0,
            "the actual minimap projection samples the subject cell"
        );
    }
    fn marker_route(kind: Mission) {
        for (owner, observer) in [(HUMAN, false), (P1, false), (P1, true)] {
            let Launched {
                host, id, power, ..
            } = launch(kind, owner, observer);
            let cell = mobile(&host, id).cell;
            let present = owner == HUMAN || observer;
            assert_eq!(has_marker(&host, cell, owner), present);
            // Parent stayed at launch cell. There is no injected view/contact/marker and no
            // overlapping marker at this early flight cell that could hide a missing subject.
            assert_eq!(
                owned_markers(&host)
                    .iter()
                    .filter(|(c, _, _)| *c == cell)
                    .count(),
                usize::from(present)
            );
            let mut world = draw_world(host);
            draw(&mut world);
            assert!(!world.resource::<MinimapState>().offline);
            assert_contact_pixels(&world, cell, owner, present);
            let events = world.resource_scope(|_, mut host: Mut<SimHost>| {
                tick(
                    &mut host,
                    vec![issue(HUMAN, Command::SellBuilding { building: power })],
                )
            });
            assert!(rejected(&events).is_empty(), "{events:?}");
            draw(&mut world);
            assert!(world.resource::<MinimapState>().offline);
            assert_eq!(world.resource::<MinimapState>().map_pixels, 0);
            assert_eq!(
                pixels(&world),
                offline_pixels(&world.resource::<Minimap>().layout)
            );
        }
    }
    #[test]
    fn ra9_b87_asw_real_mission_commands() {
        public_route(Mission::Asw);
    }
    #[test]
    fn ra9_b87_dmisl_real_mission_commands() {
        public_route(Mission::Dmisl);
    }
    #[test]
    fn ra9_b87_hornet_real_mission_commands() {
        public_route(Mission::Hornet);
    }
    #[test]
    fn ra9_b87_v3_real_mission_commands() {
        public_route(Mission::V3);
    }
    #[test]
    fn ra9_b87_pdplane_real_mission_commands() {
        public_route(Mission::Plane);
    }
    #[test]
    fn ra9_b87_asw_actual_marker_draw() {
        marker_route(Mission::Asw);
    }
    #[test]
    fn ra9_b87_dmisl_actual_marker_draw() {
        marker_route(Mission::Dmisl);
    }
    #[test]
    fn ra9_b87_hornet_actual_marker_draw() {
        marker_route(Mission::Hornet);
    }
    #[test]
    fn ra9_b87_v3_actual_marker_draw() {
        marker_route(Mission::V3);
    }
    #[test]
    fn ra9_b87_pdplane_actual_marker_draw() {
        marker_route(Mission::Plane);
    }

    fn tick_world(world: &mut World, commands: Vec<IssuedCommand>) -> Vec<SimEvent> {
        world.resource_scope(|_, mut host: Mut<SimHost>| tick(&mut host, commands))
    }
    #[test]
    fn ra9_b87_gap_owner_allied_neutral_cloak_and_real_power_sales_reach_draw() {
        for owner in [HUMAN, P1, sim::NEUTRAL] {
            for allied in [false, true] {
                let mut human_power = None;
                let mut gap_power = None;
                let mut subject = None;
                let mut scout = None;
                let mut concealed = None;
                let mut host = stock_host(48, |w| {
                    for y in 20..35 {
                        for x in 30..39 {
                            w.resource_mut::<sim::Grid>().set_terrain(
                                CellPos::new(x, y),
                                sim::Cell {
                                    passable: false,
                                    water: true,
                                    buildable: false,
                                    ore: 0,
                                },
                            );
                        }
                    }
                    if allied {
                        assert!(w.resource_mut::<sim::Players>().set_team(HUMAN, 1));
                        assert!(w.resource_mut::<sim::Players>().set_team(P1, 1));
                    }
                    human_power = Some(building(w, HUMAN, "nuclear_reactor", 1, 1));
                    building(w, HUMAN, "airforce_command", 6, 1);
                    building(w, HUMAN, "spy_satellite", 11, 1);
                    gap_power = Some(building(w, P1, "nuclear_reactor", 42, 40));
                    building(w, P1, "gap_generator", 24, 24);
                    subject = Some(unit(w, owner, "amphibious_transport", 34, 24));
                    // A real cloakable actor in visible terrain remains a separate concealment gate.
                    concealed = Some(unit(w, P1, "giant_squid", 30, 25));
                    scout = Some(unit(w, HUMAN, "amphibious_transport", 34, 28));
                });
                tick(&mut host, vec![]);
                let subject = subject.unwrap();
                let scout = scout.unwrap();
                assert_eq!(
                    mobile(&host, concealed.unwrap()).cell,
                    CellPos::new(30, 25),
                    "actual cloak fixture is at the coordinate queried by the marker checks"
                );
                assert!(
                    host.players()
                        .unwrap()
                        .get(HUMAN)
                        .unwrap()
                        .vision()
                        .is_visible(CellPos::new(34, 24)),
                    "local sight counters powered Gap"
                );
                assert!(has_marker(&host, CellPos::new(34, 24), owner));
                assert_eq!(
                    has_marker(&host, CellPos::new(30, 25), P1),
                    allied,
                    "terrain sight alone does not detect hostile stock Squid"
                );
                let mut world = draw_world(host);
                draw(&mut world);
                assert_contact_pixels(&world, CellPos::new(34, 24), owner, true);
                let events = tick_world(
                    &mut world,
                    vec![issue(
                        HUMAN,
                        Command::Move {
                            units: vec![scout],
                            target: CellPos::new(10, 28),
                        },
                    )],
                );
                assert!(rejected(&events).is_empty());
                let mut arrived = false;
                for _ in 0..1200 {
                    if mobile(world.resource::<SimHost>(), scout).cell == CellPos::new(10, 28) {
                        arrived = true;
                        break;
                    }
                    tick_world(&mut world, vec![]);
                }
                assert!(arrived, "real scout left the Gap circle");
                let host = world.resource::<SimHost>();
                let visible = owner == HUMAN || allied;
                assert_eq!(
                    host.players()
                        .unwrap()
                        .get(HUMAN)
                        .unwrap()
                        .vision()
                        .is_explored(CellPos::new(34, 24)),
                    visible,
                    "powered hostile Gap erases exploration after local sight actually leaves"
                );
                assert_eq!(has_marker(host, mobile(host, subject).cell, owner), visible);
                assert_eq!(has_marker(host, CellPos::new(30, 25), P1), allied);
                draw(&mut world);
                assert_contact_pixels(&world, CellPos::new(34, 24), owner, visible);
                // Enemy owner's genuine sale takes Gap offline while HUMAN keeps powered radar
                // and satellite. No direct power/resource mutation or Gap-state forgery.
                let events = tick_world(
                    &mut world,
                    vec![issue(
                        P1,
                        Command::SellBuilding {
                            building: gap_power.unwrap(),
                        },
                    )],
                );
                assert!(rejected(&events).is_empty());
                assert!(
                    world
                        .resource::<SimHost>()
                        .players()
                        .unwrap()
                        .get(HUMAN)
                        .unwrap()
                        .vision()
                        .is_visible(CellPos::new(34, 24))
                );
                assert!(has_marker(
                    world.resource::<SimHost>(),
                    CellPos::new(34, 24),
                    owner
                ));
                assert_eq!(
                    has_marker(world.resource::<SimHost>(), CellPos::new(30, 25), P1),
                    allied,
                    "satellite restoration is not a hostile-cloak detector"
                );
                draw(&mut world);
                assert_contact_pixels(&world, CellPos::new(34, 24), owner, true);
                let events = tick_world(
                    &mut world,
                    vec![issue(
                        HUMAN,
                        Command::SellBuilding {
                            building: human_power.unwrap(),
                        },
                    )],
                );
                assert!(rejected(&events).is_empty());
                draw(&mut world);
                assert!(world.resource::<MinimapState>().offline);
                assert_eq!(world.resource::<MinimapState>().map_pixels, 0);
                assert_eq!(
                    pixels(&world),
                    offline_pixels(&world.resource::<Minimap>().layout)
                );
            }
        }
    }
    #[test]
    fn ra9_b87_stock_sensors_four_five_cells_hide_and_show_squid_through_public_admission() {
        for detector_name in ["dolphin", "typhoon", "giant_squid"] {
            let mut target = None;
            let mut detector = None;
            let mut armed = None;
            let mut host = stock_host(0, |w| {
                building(w, HUMAN, "nuclear_reactor", 1, 1);
                building(w, HUMAN, "airforce_command", 6, 1);
                target = Some(unit(w, P1, "giant_squid", 10, 10));
                unit(w, HUMAN, "amphibious_transport", 13, 10); // Sight6, no Sensors, beyond grab1.5.
                detector = Some(unit(w, HUMAN, detector_name, 20, 10));
                armed = Some(unit(w, HUMAN, "destroyer", 30, 30)); // Sensor8 stays far away.
            });
            tick(&mut host, vec![]);
            let target = target.unwrap();
            let detector = detector.unwrap();
            let armed = armed.unwrap();
            assert_eq!(
                host.definitions()
                    .unwrap()
                    .metadata(sim::ProductionItem::Unit(
                        host.definitions()
                            .unwrap()
                            .unit_kind(detector_name)
                            .unwrap()
                    ))
                    .unwrap()
                    .sensors_sight,
                4
            );
            assert!(
                host.players()
                    .unwrap()
                    .get(HUMAN)
                    .unwrap()
                    .vision()
                    .is_visible(CellPos::new(10, 10))
            );
            assert!(!has_marker(&host, CellPos::new(10, 10), P1));
            let events = tick(
                &mut host,
                vec![issue(
                    HUMAN,
                    Command::Attack {
                        units: vec![armed],
                        target,
                    },
                )],
            );
            assert_eq!(rejected(&events), vec![RejectReason::UnknownTarget(target)]);
            let mut world = draw_world(host);
            draw(&mut world);
            assert_contact_pixels(&world, CellPos::new(10, 10), P1, false);
            // Destination lies beyond the detector boundary. Sampling at x14 occurs during
            // a real continuing move, so this armed detector has not stopped to auto-fire.
            let events = tick_world(
                &mut world,
                vec![issue(
                    HUMAN,
                    Command::Move {
                        units: vec![detector],
                        target: CellPos::new(6, 10),
                    },
                )],
            );
            assert!(rejected(&events).is_empty());
            let mut at_four = false;
            for _ in 0..500 {
                if mobile(world.resource::<SimHost>(), detector).cell == CellPos::new(14, 10) {
                    at_four = true;
                    break;
                }
                let events = tick_world(&mut world, vec![]);
                assert!(
                    !events
                        .iter()
                        .any(|e| matches!(e,SimEvent::WeaponFired{attacker,..}
                    if *attacker==detector)),
                    "continuing move isolates sensors, not a shot surface hook"
                );
            }
            assert!(at_four, "actual current-cell SensorsSight=4 boundary");
            assert!(has_marker(
                world.resource::<SimHost>(),
                CellPos::new(10, 10),
                P1
            ));
            draw(&mut world);
            assert_contact_pixels(&world, CellPos::new(10, 10), P1, true);
            let events = tick_world(
                &mut world,
                vec![
                    issue(
                        HUMAN,
                        Command::Attack {
                            units: vec![armed],
                            target,
                        },
                    ),
                    issue(
                        HUMAN,
                        Command::Move {
                            units: vec![detector],
                            target: CellPos::new(20, 10),
                        },
                    ),
                ],
            );
            assert!(
                rejected(&events).is_empty(),
                "actual observed target admitted: {events:?}"
            );
            // Stop the distant attacker before it can contribute its own SensorsSight=8.
            let events = tick_world(
                &mut world,
                vec![issue(HUMAN, Command::Stop { units: vec![armed] })],
            );
            assert!(rejected(&events).is_empty());
            let mut at_five = false;
            for _ in 0..300 {
                if mobile(world.resource::<SimHost>(), detector).cell == CellPos::new(15, 10) {
                    at_five = true;
                    break;
                }
                tick_world(&mut world, vec![]);
            }
            assert!(at_five);
            let host = world.resource::<SimHost>();
            assert_eq!(
                mobile(host, target).cell,
                CellPos::new(10, 10),
                "stock Squid stayed outside grab reach"
            );
            assert!(
                host.players()
                    .unwrap()
                    .get(HUMAN)
                    .unwrap()
                    .vision()
                    .is_visible(CellPos::new(10, 10))
            );
            assert!(
                !has_marker(host, CellPos::new(10, 10), P1),
                "visible terrain is insufficient at distance5"
            );
            let events = tick_world(
                &mut world,
                vec![issue(
                    HUMAN,
                    Command::Attack {
                        units: vec![armed],
                        target,
                    },
                )],
            );
            assert_eq!(rejected(&events), vec![RejectReason::UnknownTarget(target)]);
            draw(&mut world);
            assert_contact_pixels(&world, CellPos::new(10, 10), P1, false);
        }
    }
    #[test]
    fn ra9_b87_normal_stock_squid_attachment_and_sonic_reappearance_use_real_host_and_draw() {
        let mut source = None;
        let mut victim = None;
        let mut dolphin = None;
        let mut host = stock_host(0, |w| {
            building(w, HUMAN, "nuclear_reactor", 1, 1);
            building(w, HUMAN, "airforce_command", 6, 1);
            source = Some(unit(w, HUMAN, "giant_squid", 8, 8));
            victim = Some(unit(w, P1, "dreadnought", 9, 8));
            unit(w, HUMAN, "amphibious_transport", 12, 10); // Continues honest target observation.
            dolphin = Some(unit(w, HUMAN, "dolphin", 20, 8)); // Initially outside Sonic range8.
        });
        let source = source.unwrap();
        let victim = victim.unwrap();
        let dolphin = dolphin.unwrap();
        for p in [HUMAN, P1] {
            assert_eq!(
                host.players()
                    .unwrap()
                    .get(p)
                    .unwrap()
                    .vision()
                    .visible_cells(),
                0
            );
        }
        let primed = tick(
            &mut host,
            vec![
                issue(
                    HUMAN,
                    Command::Guard {
                        units: vec![source, dolphin],
                    },
                ),
                issue(
                    P1,
                    Command::Guard {
                        units: vec![victim],
                    },
                ),
            ],
        );
        assert!(rejected(&primed).is_empty(), "{primed:?}");
        assert!(
            !primed
                .iter()
                .any(|e| matches!(e,SimEvent::WeaponFired{attacker,..}if *attacker==source))
        );
        assert!(
            host.world()
                .resource::<sim::SpecialState>()
                .parasites
                .is_empty()
        );
        entity(&host, source); // Native existence before intended public Attack.
        let full = host
            .world()
            .get::<sim::Health>(entity(&host, source))
            .unwrap()
            .hit_points;
        assert!(has_marker(&host, CellPos::new(8, 8), HUMAN));
        let mut world = draw_world(host);
        draw(&mut world);
        assert_contact_pixels(&world, CellPos::new(8, 8), HUMAN, true);
        let events = tick_world(
            &mut world,
            vec![issue(
                HUMAN,
                Command::Attack {
                    units: vec![source],
                    target: victim,
                },
            )],
        );
        assert!(rejected(&events).is_empty(), "{events:?}");
        assert!(
            events
                .iter()
                .any(|e| matches!(e,SimEvent::WeaponFired{attacker,target,..}
            if *attacker==source&&*target==victim))
        );
        let host = world.resource::<SimHost>();
        assert_eq!(
            host.world()
                .resource::<sim::SpecialState>()
                .parasites
                .get(&victim)
                .map(|p| p.passenger.id),
            Some(source)
        );
        assert!(
            !host
                .world()
                .try_query::<&SimId>()
                .unwrap()
                .iter(host.world())
                .any(|id| *id == source)
        );
        assert!(!has_marker(host, CellPos::new(8, 8), HUMAN));
        draw(&mut world);
        assert_contact_pixels(&world, CellPos::new(8, 8), HUMAN, false);
        let events = tick_world(
            &mut world,
            vec![issue(
                HUMAN,
                Command::Move {
                    units: vec![source],
                    target: CellPos::new(5, 5),
                },
            )],
        );
        assert_eq!(rejected(&events), vec![RejectReason::UnknownUnit(source)]);
        let mut events = tick_world(
            &mut world,
            vec![issue(
                HUMAN,
                Command::Attack {
                    units: vec![dolphin],
                    target: victim,
                },
            )],
        );
        assert!(rejected(&events).is_empty());
        let mut freed = false;
        let mut sonic_shot = false;
        for _ in 0..240 {
            sonic_shot |= events.iter().any(|e| {
                matches!(e,SimEvent::WeaponFired{attacker,target,..}
                if *attacker==dolphin&&*target==victim)
            });
            freed = !world
                .resource::<SimHost>()
                .world()
                .resource::<sim::SpecialState>()
                .parasites
                .contains_key(&victim);
            if freed {
                break;
            }
            events = tick_world(&mut world, vec![]);
        }
        assert!(
            freed && sonic_shot,
            "real approach/sonic hit must release normal unprotected stock host"
        );
        let host = world.resource::<SimHost>();
        let restored = entity(host, source);
        assert_eq!(
            host.world()
                .get::<sim::Health>(restored)
                .unwrap()
                .hit_points,
            full
        );
        assert_eq!(host.world().get::<sim::Owner>(restored).unwrap().0, HUMAN);
        assert_eq!(
            host.world().get::<sim::Unit>(restored).unwrap().kind,
            host.definitions()
                .unwrap()
                .unit_kind("giant_squid")
                .unwrap()
        );
        let restored_cell = mobile(host, source).cell;
        assert!(has_marker(host, restored_cell, HUMAN));
        draw(&mut world);
        assert_contact_pixels(&world, restored_cell, HUMAN, true);
    }
    // Test-only running scenario constructor. Real setup_match_configured installs MatchState,
    // countries and both bases; no fabricated running/viewer state or mutable-host API.
    pub(crate) fn c62_running_stock_host(
        water_from: i32,
        setup: impl FnOnce(&mut World),
    ) -> SimHost {
        crate::sim_host::tests::fixture_host(|w| {
            let dir =
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data");
            let d = sim::load_ruleset(&dir).unwrap();
            let mut skirmish = sim::load_skirmish(dir.join("skirmish.ron"), &d).unwrap();
            let ai = sim::load_ai(dir.join("ai.ron"), &d).unwrap();
            // Independent test geometry only. Both real bases are far from the launch corridor.
            let cells = (0..48)
                .flat_map(|y| {
                    (0..48).map(move |x| {
                        let water = x >= water_from && !(x <= 8 && y <= 5) && !(x >= 40 && y >= 38);
                        sim::Cell {
                            passable: !water,
                            water,
                            buildable: !water,
                            ore: 0,
                        }
                    })
                })
                .collect();
            w.insert_resource(sim::Grid::from_cells(48, 48, cells).unwrap());
            w.insert_resource(d);
            skirmish.maps[0].starts = vec![CellPos::new(2, 40), CellPos::new(42, 40)];
            let players = [("great_britain", HUMAN), ("russia", P1)]
                .into_iter()
                .enumerate()
                .map(|(start, (country, player))| sim::PlayerSetup {
                    player,
                    start,
                    controller: sim::Controller::Human,
                    faction: skirmish
                        .factions
                        .iter()
                        .position(|f| f.id == country)
                        .unwrap(),
                })
                .collect::<Vec<_>>();
            let options = sim::MatchOptions {
                unit_count: Some(0),
                mcv_start: false,
                crates: false,
                short_game: false,
                free_for_all: true,
                ..Default::default()
            };
            sim::setup_match_configured(w, &skirmish, &ai, 0, &players, &options).unwrap();
            setup(w);
        })
    }
    /// Actual command-created mission plus real current owned ordinary actors for input tests.
    pub(crate) fn c62_mission(section: &str) -> (SimHost, SimId, SimId, Vec<SimId>) {
        let kind = match section {
            "DMISL" => Mission::Dmisl,
            "V3ROCKET" => Mission::V3,
            "PDPLANE" => Mission::Plane,
            _ => panic!("test section"),
        };
        let Launched { host, id, .. } = launch_in(kind, HUMAN, false, true);
        assert!(host.running());
        let engineer = host.definitions().unwrap().unit_kind("engineer").unwrap();
        let mut q = host
            .world()
            .try_query::<(&SimId, &sim::Owner, &sim::Unit, &sim::Health)>()
            .unwrap();
        let mut eligible = Vec::new();
        let mut normal = None;
        for (found, owner, u, hp) in q.iter(host.world()) {
            if owner.0 == HUMAN && hp.hit_points > 0 && *found != id {
                eligible.push(*found);
                if u.kind == engineer && normal.is_none() {
                    normal = Some(*found);
                }
            }
        }
        eligible.sort();
        assert_eq!(eligible.len(), 2, "real zero-start-unit scenario actors");
        (host, id, normal.unwrap(), eligible)
    }
    pub(crate) fn c62_wall(name: &str) -> (SimHost, SimId, SimId, SimId, Vec<SimId>) {
        assert!(matches!(
            name,
            "fortress_walls" | "soviet_walls" | "bridge_hut"
        ));
        let mut wall = None;
        let mut normal = None;
        let mut factory = None;
        let mut fighter = None;
        let mut host = c62_running_stock_host(48, |w| {
            building(w, HUMAN, "nuclear_reactor", 1, 1);
            wall = Some(building(w, HUMAN, name, 18, 14));
            normal = Some(unit(w, HUMAN, "engineer", 20, 20));
            fighter = Some(unit(w, HUMAN, "grizzly", 21, 20));
            factory = Some(building(w, HUMAN, "war_factory", 14, 14));
        });
        tick(&mut host, vec![]);
        assert!(host.running());
        let normal = normal.unwrap();
        let mut eligible = vec![normal, fighter.unwrap()];
        eligible.sort();
        (host, wall.unwrap(), normal, factory.unwrap(), eligible)
    }
}
