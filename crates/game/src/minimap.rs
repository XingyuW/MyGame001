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
//! Layout: continuous cell coordinates (u, v) of a W × H map map to the normalised image point
//! `((u − v + H) / (W + H), (u + v) / (W + H))`, the main view's projection (equation 7) scaled
//! so that the map's bounding rectangle fills the image. So the camera's view, an axis-aligned
//! rectangle in the world, is an axis-aligned rectangle on the minimap.

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
                    (draw_minimap, place_camera_frame)
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
            scale: (MINIMAP_TARGET_WIDTH / sum).max(1),
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
pub fn cell_colours(
    terrain: &Grid,
    vision: Option<&Vision>,
    markers: &[(CellPos, Marker)],
) -> Vec<[u8; 4]> {
    let (width, height) = (terrain.width(), terrain.height());
    let mut colours = Vec::with_capacity(width as usize * height as usize);
    for y in 0..height {
        for x in 0..width {
            let cell = CellPos::new(x as i32, y as i32);
            let sight = vision.map_or(Sight::Shroud, |vision| sight_of(vision, cell));
            let colour = terrain
                .cell(cell)
                .map_or([0, 0, 0, 255], |terrain| terrain_colour(terrain, sight));
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
pub fn markers(host: &SimHost) -> Vec<(CellPos, Marker)> {
    let Some(players) = host.players() else {
        return Vec::new();
    };
    let mut markers = Vec::new();
    if let Some(mut units) = host.world().try_query::<(&Owner, &Mobile)>() {
        for (owner, mobile) in units.iter(host.world()) {
            if crate::units::unit_shown(owner.0, mobile, players) {
                let marker = if owner.0 == HUMAN {
                    Marker::OwnUnit
                } else {
                    Marker::EnemyUnit
                };
                markers.push((mobile.cell, marker));
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
                markers.extend(building.footprint.cells().map(|cell| (cell, marker)));
            }
        }
    }
    // Units are drawn over buildings (a unit never stands in a building's cell, I6).
    markers.sort_by_key(|(_, marker)| matches!(marker, Marker::OwnUnit | Marker::EnemyUnit));
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
    /// The simulation tick of the last drawing.
    pub drawn_tick: Option<u64>,
    /// Drawings so far.
    pub drawings: u64,
    /// Pixels on the map in the last drawing that are not black.
    pub lit_pixels: usize,
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
fn draw_minimap(
    host: Res<SimHost>,
    terrain: Option<Res<MapTerrain>>,
    minimap: Option<Res<Minimap>>,
    mut state: ResMut<MinimapState>,
    mut images: ResMut<Assets<Image>>,
) {
    let (Some(terrain), Some(minimap)) = (terrain, minimap) else {
        return;
    };
    let tick = host.tick();
    if state.drawn_tick == Some(tick) {
        return;
    }
    let colours = cell_colours(&terrain.0, human_vision(host.players()), &markers(&host));
    let pixels = render(&minimap.layout, &colours);
    state.lit_pixels = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[3] > 0 && pixel[..3] != [0, 0, 0])
        .count();
    if let Some(mut image) = images.get_mut(&minimap.image)
        && image.data.as_ref() != Some(&pixels)
    {
        image.data = Some(pixels);
    }
    state.drawn_tick = Some(tick);
    state.drawings = state.drawings.saturating_add(1);
}

/// Places the camera's rectangle on the minimap.
pub(crate) fn place_camera_frame(
    map: Option<Res<MapView>>,
    minimap: Option<Res<Minimap>>,
    camera: Single<(&Camera, &Projection, &Transform), With<Camera2d>>,
    mut frames: Query<&mut Node, With<MinimapFrame>>,
) {
    let (Some(map), Some(minimap)) = (map, minimap) else {
        return;
    };
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
/// point under the cursor.
fn click_minimap(
    buttons: Res<ButtonInput<MouseButton>>,
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
    if !*dragging || window.cursor_position().is_none() {
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
