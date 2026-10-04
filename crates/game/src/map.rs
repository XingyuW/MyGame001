//! Map loading, rendering, and cursor picking (PLAN.md section 7, M1).
//!
//! The map file is validated by `sim::load_map_with_files` before anything else touches it: the
//! raw file and its external tileset are restricted to the supported subset of the Tiled format,
//! which excludes every input found by M1 reviews 1 and 2 to make `tiled`, `bevy_ecs_tiled`, or
//! `bevy_asset` panic, recurse without bound, allocate without bound, or exceed the renderer's
//! texture limits (docs/DECISIONS.md, D13 and D16; invariant I11). The tileset image is then
//! restricted to its own subset and decoded once under a memory limit
//! ([`crate::tileset_image`]). Only a map that passed both is handed to `bevy_ecs_tiled`, and the
//! asset server is given exactly the checked bytes of these files and nothing else
//! ([`crate::checked_assets`]). This is a claim about the inputs those checks cover, verified by
//! the tests in `crates/sim/tests/m1_map_subset.rs`, `crates/game/tests/m1_tileset_image.rs`, and
//! below, not a proof that the third party code cannot fail; any asset load failure ends the game
//! with exit code 1 (`exit_on_asset_load_failure`).
//!
//! Every map of the skirmish file is checked this way once, when the game starts
//! (`crate::skirmish`); a match installs the checked grid into the simulation, and the map is
//! rendered through `bevy_ecs_tiled` from the checked bytes at the offset that aligns it with
//! [`crate::projection`] ([`spawn_map`]).

use std::path::Path;

use bevy::asset::UntypedAssetLoadFailedEvent;
use bevy::camera::CameraUpdateSystems;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use bevy::window::PrimaryWindow;
use bevy_ecs_tiled::prelude::*;
use sim::{CellPos, Grid};

use crate::checked_assets::CheckedAssets;
use crate::projection::{TileSize, cell_centre, cell_corners, map_origin_offset, pick_cell};
use crate::tileset_image::check_tileset_image;

/// The demo map, relative to the asset directory (the tests' map; the game's maps are those of
/// the skirmish file, M6).
#[cfg(test)]
pub const MAP_PATH: &str = "maps/demo/demo.tmx";

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapStatus>()
            .init_resource::<HoveredCell>()
            .add_systems(Update, on_map_created)
            .add_systems(Last, exit_on_asset_load_failure)
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

/// What has happened to the map of the current match so far; read by the smoke test.
#[derive(Resource, Debug, Default)]
pub struct MapStatus {
    pub map_created: bool,
}

/// The map entity of the current match; despawned, with the tiles `bevy_ecs_tiled` spawned under
/// it, when the match ends.
#[derive(Component, Debug)]
pub struct MapRoot;

/// The map file of the current match, relative to the asset directory.
#[derive(Resource, Clone, Debug)]
pub struct MapAsset(pub std::path::PathBuf);

/// The map cell under the mouse cursor, if any.
#[derive(Resource, Debug, Default)]
pub struct HoveredCell(pub Option<CellPos>);

/// The terrain of the map as its file defines it (passable, buildable, ore at the start), as the
/// tiles draw it. The interface uses it for what the player knows of a cell: it never shows the
/// simulation's current ore or occupancy of cells the player does not see (M5; docs/DECISIONS.md
/// D36).
#[derive(Resource, Clone, Debug)]
pub struct MapTerrain(pub Grid);

/// A map whose files passed their checks: its grid and the size the renderer needs.
#[derive(Clone, Debug)]
pub struct CheckedMap {
    pub grid: Grid,
    pub view: MapView,
}

/// Checks the map at `asset_path` (relative to `root`, the asset directory) and its tileset image
/// before anything else touches them (D13, D16), and registers exactly the checked bytes for the
/// renderer under their asset paths. Errors name the file, the field, and the reason.
pub fn check_map(
    root: &Path,
    asset_path: &Path,
    checked: &CheckedAssets,
) -> Result<CheckedMap, String> {
    let path = root.join(asset_path);
    let (loaded, files) = sim::load_map_with_files(&path).map_err(|error| error.to_string())?;
    let image =
        check_tileset_image(&path, &loaded.tileset_image).map_err(|error| error.to_string())?;
    // The renderer reads exactly the checked bytes (D16). Every checked file lies in the map's
    // directory (D13), so it has the same asset path as on disk.
    let directory = asset_path.parent().unwrap_or(Path::new(""));
    for file in files {
        if let Some(name) = file.path.file_name() {
            checked.insert(directory.join(name), file.bytes);
        }
    }
    checked.insert(directory.join(&loaded.tileset_image.file_name), image);
    let view = MapView {
        width: loaded.grid.width(),
        height: loaded.grid.height(),
        tile: TileSize::new(loaded.tile_width, loaded.tile_height),
    };
    info!(
        "map {}: {}x{} cells, tile {}x{} px",
        asset_path.display(),
        view.width,
        view.height,
        loaded.tile_width,
        loaded.tile_height
    );
    Ok(CheckedMap {
        grid: loaded.grid,
        view,
    })
}

/// Spawns the map of a match from its checked bytes, with the resources that the rendering,
/// picking, and interface systems read (`MapView`, `MapTerrain`, `MapAsset`). The game keeps the
/// terrain as the map file defines it, which is what the player sees of a cell, also under fog
/// (`MapTerrain`).
pub(crate) fn spawn_map(world: &mut World, asset_path: &Path, map: &CheckedMap) {
    let handle = world
        .resource::<AssetServer>()
        .load(asset_path.to_path_buf());
    world.insert_resource(map.view);
    world.insert_resource(MapTerrain(map.grid.clone()));
    world.insert_resource(MapAsset(asset_path.to_path_buf()));
    world.insert_resource(MapStatus::default());
    world.spawn((
        MapRoot,
        TiledMap(handle),
        TilemapAnchor::None,
        Transform::from_translation(map_origin_offset(map.view.height, map.view.tile).extend(0.0)),
    ));
}

/// Removes the map of a match: its entity with everything spawned under it, and its resources.
pub(crate) fn despawn_map(world: &mut World) {
    let roots: Vec<Entity> = world
        .query_filtered::<Entity, With<MapRoot>>()
        .iter(world)
        .collect();
    for root in roots {
        world.despawn(root);
    }
    world.remove_resource::<MapView>();
    world.remove_resource::<MapTerrain>();
    world.remove_resource::<MapAsset>();
    world.insert_resource(MapStatus::default());
    world.insert_resource(HoveredCell::default());
}

/// Any asset that fails to load (the map, its tileset image, or a later asset) ends the game
/// with exit code 1 in every mode, as a map rejected by `sim` does. Without it a normal run kept
/// going without a map (M1 review 1, P1-2).
pub fn exit_on_asset_load_failure(
    mut failures: MessageReader<UntypedAssetLoadFailedEvent>,
    mut exit: MessageWriter<AppExit>,
) {
    for failure in failures.read() {
        error!("asset {} failed to load: {}", failure.path, failure.error);
        exit.write(AppExit::error());
    }
}

/// Records map creation and rejects maps whose rendering would not line up with the
/// projection.
fn on_map_created(
    mut created: MessageReader<TiledEvent<MapCreated>>,
    maps: Res<Assets<TiledMapAsset>>,
    asset: Option<Res<MapAsset>>,
    mut status: ResMut<MapStatus>,
    mut exit: MessageWriter<AppExit>,
) {
    let name = asset.map_or_else(String::new, |asset| asset.0.display().to_string());
    for event in created.read() {
        let Some(map) = event.get_map(&maps) else {
            error!("map {name} was created but its asset is not available");
            exit.write(AppExit::error());
            continue;
        };
        if let Some(problem) = misalignment(map) {
            error!("map {name}: {problem}");
            exit.write(AppExit::error());
            continue;
        }
        status.map_created = true;
    }
}

/// `bevy_ecs_tiled` applies layer offsets, layer parallax, and tileset offsets on top of the
/// tile positions; [`map_origin_offset`] assumes all of them are neutral. Returns a description
/// of the first one that is not. `sim` rejects them before the hand-off (D13); this is the second
/// line of defence (D9).
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

pub(crate) fn update_hovered_cell(
    view: Option<Res<MapView>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform)>,
    mut hovered: ResMut<HoveredCell>,
) {
    let (camera, camera_transform) = *camera;
    hovered.0 = view.and_then(|view| {
        // The sidebar (M5) covers the map on the right; no cell is under it.
        let cursor = window
            .cursor_position()
            .filter(|&cursor| !crate::sidebar::over_sidebar(cursor, window.width()))?;
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
    terrain: Option<Res<MapTerrain>>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    let over_sidebar = window
        .cursor_position()
        .is_some_and(|cursor| crate::sidebar::over_sidebar(cursor, window.width()));
    if !buttons.just_pressed(MouseButton::Left) || over_sidebar {
        return;
    }
    let Some(terrain) = terrain else {
        return;
    };
    match hovered
        .0
        .and_then(|pos| terrain.0.cell(pos).map(|cell| (pos, cell)))
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

    use bevy::asset::io::AssetReaderError;
    use bevy::asset::{AssetId, AssetLoadError, AssetPath};
    use sim::TilesetImage;

    use super::*;

    fn asset_path(relative: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join(relative)
    }

    /// An empty directory unique to this test process and `name`, so that concurrent test runs
    /// do not share files (M1 review 1, P2-4).
    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "mygame001_game_map_tests_{}_{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temporary directory");
        dir
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
        let dir = scratch_dir("misalignment");
        for (name, from, to, expected) in cases {
            assert_eq!(text.matches(from).count(), 1, "{name}");
            let path = dir.join(format!("{name}.tmx"));
            std::fs::write(&path, text.replacen(from, &to, 1)).expect("temporary map");
            let problem = misalignment(&load_tiled(&path)).expect(name);
            assert!(problem.contains(expected), "{name}: {problem}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn demo_tileset_image_matches_its_declared_size() {
        let path = asset_path(MAP_PATH);
        let loaded = sim::load_map(&path).unwrap_or_else(|error| panic!("{error}"));
        // The checked bytes are returned, so that the renderer receives exactly them (D16).
        let file = std::fs::read(asset_path("maps/demo/demo_tiles.png")).expect("demo image");
        assert_eq!(check_tileset_image(&path, &loaded.tileset_image), Ok(file));
    }

    #[test]
    fn tileset_images_other_than_the_declared_png_are_rejected() {
        // Regression (M1 review 1, P1-1): a 1x1 image in place of a declared 512x32 one was
        // rendered without any error.
        let dir = scratch_dir("images");
        let map = dir.join("map.tmx");
        let image = dir.join("tiles.png");
        let declared = TilesetImage {
            file_name: "tiles.png".into(),
            width: 512,
            height: 32,
        };
        let png_header = |width: u32, height: u32| {
            let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
            bytes.extend(width.to_be_bytes());
            bytes.extend(height.to_be_bytes());
            bytes
        };
        let rejected = |contents: Option<&[u8]>, expected: &str| {
            if let Some(contents) = contents {
                std::fs::write(&image, contents).expect("image");
            }
            let problem = check_tileset_image(&map, &declared)
                .expect_err(expected)
                .to_string();
            assert!(problem.contains(expected), "{problem}");
            assert!(problem.contains(&image.display().to_string()), "{problem}");
        };
        rejected(None, "cannot be read");
        rejected(
            Some(&png_header(1, 1)),
            "the image is 1x1 px, but the map declares 512x32 px",
        );
        rejected(
            Some(&png_header(512, 33)),
            "the image is 512x33 px, but the map declares 512x32 px",
        );
        rejected(
            Some(b"GIF89a\x00\x02\x20\x00 not a png at all"),
            "not a PNG file",
        );
        rejected(Some(&png_header(512, 32)[..20]), "cannot be read");
        // M1 review 2, P0-A: only the header was read, so a file holding nothing but a header of
        // the right size was accepted. The whole file is now checked and decoded.
        rejected(
            Some(&png_header(512, 32)),
            r#"chunk "IHDR": the file ends inside this chunk"#,
        );
        let mut complete = Vec::new();
        image::RgbaImage::new(512, 32)
            .write_to(
                &mut std::io::Cursor::new(&mut complete),
                image::ImageFormat::Png,
            )
            .expect("encoded PNG");
        std::fs::write(&image, &complete).expect("image");
        assert_eq!(check_tileset_image(&map, &declared), Ok(complete));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tileset_limits_fit_the_texture_limits_every_gpu_supports() {
        // Regression (M1 review 2, P1-A): `sim` accepted tileset images and tile counts that
        // exceed the renderer's texture limits. The tileset image is uploaded as one 2D texture,
        // and `bevy_ecs_tilemap` copies the tiles into a texture array with one layer per tile,
        // plus one when the count is 1 or a multiple of 6 (`texture_array_cache.rs`).
        let guaranteed = bevy::render::settings::WgpuLimits::default();
        assert!(sim::MAX_TILESET_IMAGE_SIDE_PX <= guaranteed.max_texture_dimension_2d);
        let layers = (1..=sim::MAX_TILESET_TILES)
            .map(|count| {
                if count == 1 || count % 6 == 0 {
                    count + 1
                } else {
                    count
                }
            })
            .max();
        assert!(layers <= Some(guaranteed.max_texture_array_layers));
    }

    #[test]
    fn an_asset_load_failure_ends_the_game_with_an_error() {
        // Regression (M1 review 1, P1-2): outside `--smoke-test`, nothing reacted to a failed
        // map load, and the game kept running without a map.
        let mut app = App::new();
        app.add_message::<UntypedAssetLoadFailedEvent>()
            .add_message::<AppExit>()
            .add_systems(Last, exit_on_asset_load_failure);
        app.update();
        assert_eq!(app.should_exit(), None);
        app.world_mut().write_message(UntypedAssetLoadFailedEvent {
            id: AssetId::<Image>::invalid().untyped(),
            path: AssetPath::from(MAP_PATH),
            error: AssetLoadError::AssetReaderError(AssetReaderError::NotFound(MAP_PATH.into())),
        });
        app.update();
        assert_eq!(app.should_exit(), Some(AppExit::error()));
    }
}
