//! Map loading, rendering, and cursor picking (PLAN.md section 7, M1).
//!
//! The map file is validated by `sim::load_map` before anything else touches it, so
//! `bevy_ecs_tiled` only ever sees maps that cannot make it panic (docs/DECISIONS.md, A2
//! caveat). The validated grid is installed into the simulation, and the same file is rendered
//! through `bevy_ecs_tiled` at the offset that aligns it with [`crate::projection`].

use bevy::asset::io::file::FileAssetReader;
use bevy::camera::CameraUpdateSystems;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use bevy::window::PrimaryWindow;
use bevy_ecs_tiled::prelude::*;
use sim::{CellPos, Grid};

use crate::camera::CameraBounds;
use crate::projection::{
    TileSize, cell_centre, cell_corners, map_bounds, map_origin_offset, pick_cell,
};

/// The map loaded at startup, relative to the asset directory. Map selection arrives in M6.
pub const MAP_PATH: &str = "maps/demo/demo.tmx";

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapStatus>()
            .init_resource::<HoveredCell>()
            .add_systems(Startup, load_map)
            .add_systems(Update, on_map_created)
            .add_systems(
                PostUpdate,
                (update_hovered_cell, draw_hovered_cell, report_clicked_cell)
                    .chain()
                    .after(TransformSystems::Propagate)
                    .after(CameraUpdateSystems),
            );
    }
}

/// Size and tile size of the loaded map, for rendering and picking.
#[derive(Resource, Clone, Copy, Debug)]
pub struct MapView {
    pub width: u32,
    pub height: u32,
    pub tile: TileSize,
}

/// What has happened to the map so far; read by the smoke test.
#[derive(Resource, Debug, Default)]
pub struct MapStatus {
    pub map_created: bool,
}

/// The map cell under the mouse cursor, if any.
#[derive(Resource, Debug, Default)]
pub struct HoveredCell(pub Option<CellPos>);

fn load_map(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut exit: MessageWriter<AppExit>,
) {
    let path = FileAssetReader::new(AssetPlugin::default().file_path)
        .root_path()
        .join(MAP_PATH);
    let loaded = match sim::load_map(&path) {
        Ok(loaded) => loaded,
        Err(error) => {
            error!("{error}");
            exit.write(AppExit::error());
            return;
        }
    };
    let view = MapView {
        width: loaded.grid.width(),
        height: loaded.grid.height(),
        tile: TileSize::new(loaded.tile_width, loaded.tile_height),
    };
    info!(
        "map {MAP_PATH}: {}x{} cells, tile {}x{} px",
        view.width, view.height, loaded.tile_width, loaded.tile_height
    );
    commands.insert_resource(CameraBounds(Some(map_bounds(
        view.width,
        view.height,
        view.tile,
    ))));
    commands.insert_resource(view);
    // Match setup: the simulation receives its grid before the first tick.
    commands.insert_resource::<Grid>(loaded.grid);
    commands.spawn((
        TiledMap(asset_server.load(MAP_PATH)),
        TilemapAnchor::None,
        Transform::from_translation(map_origin_offset(view.height, view.tile).extend(0.0)),
    ));
}

/// Records map creation and rejects maps whose rendering would not line up with the
/// projection.
fn on_map_created(
    mut created: MessageReader<TiledEvent<MapCreated>>,
    maps: Res<Assets<TiledMapAsset>>,
    mut status: ResMut<MapStatus>,
    mut exit: MessageWriter<AppExit>,
) {
    for event in created.read() {
        let Some(map) = event.get_map(&maps) else {
            error!("map {MAP_PATH} was created but its asset is not available");
            exit.write(AppExit::error());
            continue;
        };
        if let Some(problem) = misalignment(map) {
            error!("map {MAP_PATH}: {problem}");
            exit.write(AppExit::error());
            continue;
        }
        status.map_created = true;
    }
}

/// `bevy_ecs_tiled` applies layer offsets, layer parallax, and tileset offsets on top of the
/// tile positions; [`map_origin_offset`] assumes all of them are neutral. Returns a description
/// of the first one that is not. These are rendering properties, so `sim` does not check them.
pub fn misalignment(map: &tiled::Map) -> Option<String> {
    for layer in map.layers() {
        if layer.offset_x != 0.0 || layer.offset_y != 0.0 {
            return Some(format!(
                "layer {:?} has an offset ({}, {}); offsets are not supported",
                layer.name, layer.offset_x, layer.offset_y
            ));
        }
        if layer.parallax_x != 1.0 || layer.parallax_y != 1.0 {
            return Some(format!(
                "layer {:?} has parallax ({}, {}); parallax is not supported",
                layer.name, layer.parallax_x, layer.parallax_y
            ));
        }
    }
    for tileset in map.tilesets() {
        if tileset.offset_x != 0 || tileset.offset_y != 0 {
            return Some(format!(
                "tileset {:?} has a tile offset ({}, {}); offsets are not supported",
                tileset.name, tileset.offset_x, tileset.offset_y
            ));
        }
    }
    None
}

fn update_hovered_cell(
    view: Option<Res<MapView>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform)>,
    mut hovered: ResMut<HoveredCell>,
) {
    let (camera, camera_transform) = *camera;
    hovered.0 = view.and_then(|view| {
        let cursor = window.cursor_position()?;
        let world = camera.viewport_to_world_2d(camera_transform, cursor).ok()?;
        pick_cell(world, view.tile, view.width, view.height)
    });
}

fn draw_hovered_cell(hovered: Res<HoveredCell>, view: Option<Res<MapView>>, mut gizmos: Gizmos) {
    let (Some(cell), Some(view)) = (hovered.0, view) else {
        return;
    };
    // The outline comes from the cell's corners and the dot from its centre; both must sit on
    // the tile under the cursor (user verification of picking).
    let [top, right, bottom, left] = cell_corners(cell, view.tile);
    gizmos.linestrip_2d([top, right, bottom, left, top], Color::WHITE);
    gizmos.circle_2d(cell_centre(cell, view.tile), 2.0, Color::WHITE);
}

/// Logs the cell under a left click, so that picking can be checked by hand.
fn report_clicked_cell(
    buttons: Res<ButtonInput<MouseButton>>,
    hovered: Res<HoveredCell>,
    grid: Res<Grid>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    match hovered
        .0
        .and_then(|pos| grid.cell(pos).map(|cell| (pos, cell)))
    {
        Some((pos, cell)) => info!(
            "cell ({}, {}): passable {}, buildable {}, ore {}",
            pos.x, pos.y, cell.passable, cell.buildable, cell.ore
        ),
        None => info!("click outside the map"),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn asset_path(relative: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join(relative)
    }

    fn load_tiled(path: &std::path::Path) -> tiled::Map {
        tiled::Loader::new()
            .load_tmx_map(path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    }

    #[test]
    fn demo_map_is_aligned() {
        assert_eq!(misalignment(&load_tiled(&asset_path(MAP_PATH))), None);
    }

    #[test]
    fn offsets_and_parallax_are_reported() {
        let text = std::fs::read_to_string(asset_path(MAP_PATH)).expect("demo map");
        let layer = r#"<layer id="1" name="terrain""#;
        let image = r#"<image source="demo_tiles.png""#;
        let cases = [
            (
                "offset",
                layer,
                format!(r#"{layer} offsetx="4""#),
                "has an offset",
            ),
            (
                "parallax",
                layer,
                format!(r#"{layer} parallaxx="0.5""#),
                "has parallax",
            ),
            (
                "tile_offset",
                image,
                format!(r#"<tileoffset x="0" y="8"/>{image}"#),
                "has a tile offset",
            ),
        ];
        // `tiled` does not open the tileset image, so the copies can live anywhere.
        let dir = std::env::temp_dir().join("mygame001_game_map_tests");
        std::fs::create_dir_all(&dir).expect("temporary directory");
        for (name, from, to, expected) in cases {
            assert_eq!(text.matches(from).count(), 1, "{name}");
            let path = dir.join(format!("{name}.tmx"));
            std::fs::write(&path, text.replacen(from, &to, 1)).expect("temporary map");
            let problem = misalignment(&load_tiled(&path)).expect(name);
            assert!(problem.contains(expected), "{name}: {problem}");
        }
    }
}
