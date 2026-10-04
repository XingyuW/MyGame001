//! Map loading, rendering, and cursor picking (PLAN.md section 7, M1).
//!
//! The map file is validated by `sim::load_map` before anything else touches it: the raw file
//! and its external tileset are restricted to the supported subset of the Tiled format, which
//! excludes every input found by M1 review 1 to make `tiled`, `bevy_ecs_tiled`, or `bevy_asset`
//! panic, recurse without bound, or allocate without bound (docs/DECISIONS.md, D13; invariant
//! I11). The tileset image is then checked against the size the map declares for it. Only a map
//! that passed both is handed to `bevy_ecs_tiled`. This is a claim about the inputs those checks
//! cover, verified by the tests in `crates/sim/tests/m1_map_subset.rs` and below, not a proof
//! that the third party code cannot fail: an image that passes the header check but cannot be
//! decoded still fails to load, and any asset load failure ends the game with exit code 1
//! (`exit_on_asset_load_failure`).
//!
//! The validated grid is installed into the simulation, and the map is rendered through
//! `bevy_ecs_tiled` at the offset that aligns it with [`crate::projection`]. The asset server
//! reads the map file a second time (STATUS.md, M1 debt).

use std::io::Read;
use std::path::Path;

use bevy::asset::UntypedAssetLoadFailedEvent;
use bevy::asset::io::file::FileAssetReader;
use bevy::camera::CameraUpdateSystems;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use bevy::window::PrimaryWindow;
use bevy_ecs_tiled::prelude::*;
use sim::{CellPos, Grid, TilesetImage};

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
    if let Err(problem) = check_tileset_image(&path, &loaded.tileset_image) {
        error!("map {}: {problem}", path.display());
        exit.write(AppExit::error());
        return;
    }
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

/// The tileset image must be a PNG file whose size is the size the map declares for it: the
/// renderer slices the image into tiles by the declared size, so a different image would be
/// drawn wrongly without any error (M1 review 1, P1-1). Only the PNG header is read here; an
/// image that cannot be decoded fails to load, which [`exit_on_asset_load_failure`] reports.
pub fn check_tileset_image(map_path: &Path, image: &TilesetImage) -> Result<(), String> {
    let path = map_path
        .parent()
        .unwrap_or(Path::new(""))
        .join(&image.file_name);
    let mut header = [0u8; 24];
    std::fs::File::open(&path)
        .and_then(|file| file.take(24).read_exact(&mut header))
        .map_err(|error| format!("tileset image {}: cannot be read: {error}", path.display()))?;
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    // The first chunk of a PNG file is IHDR, 13 bytes long, starting with the width and height
    // as big-endian 32-bit integers.
    if &header[..8] != SIGNATURE || &header[8..16] != b"\0\0\0\x0dIHDR" {
        return Err(format!("tileset image {}: not a PNG file", path.display()));
    }
    let width = u32::from_be_bytes([header[16], header[17], header[18], header[19]]);
    let height = u32::from_be_bytes([header[20], header[21], header[22], header[23]]);
    if (width, height) != (image.width, image.height) {
        return Err(format!(
            "tileset image {}: the image is {width}x{height} px, but the map declares {}x{} px",
            path.display(),
            image.width,
            image.height
        ));
    }
    Ok(())
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

    use bevy::asset::io::AssetReaderError;
    use bevy::asset::{AssetId, AssetLoadError, AssetPath};

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
        assert_eq!(check_tileset_image(&path, &loaded.tileset_image), Ok(()));
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
            let problem = check_tileset_image(&map, &declared).expect_err(expected);
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
        std::fs::write(&image, png_header(512, 32)).expect("image");
        assert_eq!(check_tileset_image(&map, &declared), Ok(()));
        let _ = std::fs::remove_dir_all(&dir);
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
