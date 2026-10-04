//! Units in the game (PLAN.md section 7, M2): match setup from the unit definitions, the
//! placeholder sprite sheets, and drawing the simulation's units with eight facings and
//! interpolated positions.
//!
//! The game reads simulation state and never writes it (PLAN.md section 4, invariant I5): each
//! simulation unit is drawn by a separate view entity that copies what it needs from the unit's
//! components. The only writes into the simulation world are match setup before the first tick
//! (installing the definitions and spawning the starting units, like the map's grid, D11) and
//! commands pushed into `CommandQueue` by the interface (`crate::selection`).
//!
//! The unit definitions are loaded by `sim::load_unit_definitions` (the RON subset of D18). Each
//! definition names a sprite sheet, which must exist and pass the PNG subset check before the
//! match starts (cross validation, PLAN.md section 5; D20); the renderer receives exactly the
//! checked bytes (`crate::checked_assets`).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use bevy::asset::io::file::FileAssetReader;
use bevy::prelude::*;
use sim::{
    CellPos, DataError, Definitions, Direction, Facing, Grid, Owner, PlayerId, Position,
    SUBCELLS_PER_CELL, SimEvent, SimId, SubPos, Unit, UnitKind, destination_cells,
};

use crate::checked_assets::CheckedAssets;
use crate::map::MapView;
use crate::projection::{TileSize, to_world};
use crate::tileset_image::{TilesetImageError, TilesetImageErrorKind, check_png_file};

/// The units file, relative to the asset directory.
pub const UNITS_PATH: &str = "data/units.ron";

/// Directory of the unit sprite sheets, relative to the asset directory.
pub const SPRITE_DIR: &str = "sprites/units";

/// Width and height of one sprite frame in pixels.
pub const FRAME_SIZE: u32 = 64;

/// Frames per sprite sheet: one per facing, in the order of `sim::Direction::ALL`.
pub const FACINGS: u32 = 8;

/// Pixels from the centre of a frame down to the ground point under the unit's centre.
pub const GROUND_OFFSET_PX: f32 = 12.0;

/// The player who uses this interface; the other player's units cannot be selected.
pub const HUMAN: PlayerId = PlayerId(0);

/// The other player, whose units stand on the map from M2 on and act from M6 on.
pub const OPPONENT: PlayerId = PlayerId(1);

/// Lowest z of a unit sprite. The map's layers lie at z = 0 (`bevy_ecs_tiled`).
const UNIT_Z: f32 = 1.0;

/// A group of starting units: owner, unit type, the cell they are placed around, and how many.
pub type StartingGroup = (PlayerId, &'static str, (i32, i32), usize);

/// The starting units of the demo match (M2). Match setup arrives in M6.
const STARTING_UNITS: [StartingGroup; 4] = [
    (HUMAN, "light_tank", (10, 28), 12),
    (HUMAN, "scout", (14, 34), 6),
    (OPPONENT, "light_tank", (50, 30), 5),
    (OPPONENT, "scout", (52, 36), 3),
];

/// The number of starting units of the demo match, all of which match setup must spawn.
pub fn starting_unit_total() -> usize {
    STARTING_UNITS.iter().map(|&(.., count)| count).sum()
}

pub struct UnitsPlugin;

impl Plugin for UnitsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UnitViews>()
            .init_resource::<UnitStatus>()
            .add_systems(Startup, setup_units.after(crate::map::load_map))
            .add_systems(Update, (spawn_unit_views, log_sim_events))
            .add_systems(
                PostUpdate,
                place_unit_views.before(bevy::transform::TransformSystems::Propagate),
            );
    }
}

/// The sprite sheet of each unit type.
#[derive(Resource, Debug, Default)]
pub struct UnitSprites(pub BTreeMap<UnitKind, Handle<Image>>);

/// The view entity of each simulation unit.
#[derive(Resource, Debug, Default)]
pub struct UnitViews(pub BTreeMap<SimId, Entity>);

/// What has happened to the units so far; read by the smoke test.
#[derive(Resource, Debug, Default)]
pub struct UnitStatus {
    /// Units spawned by match setup.
    pub spawned: usize,
}

/// The entity that draws a simulation unit.
#[derive(Component, Debug)]
pub struct UnitView {
    /// The simulation entity.
    pub unit: Entity,
    pub id: SimId,
    pub owner: PlayerId,
    /// The unit's position at the start of the current tick, for interpolation.
    pub previous: SubPos,
    /// The unit's position after the last tick.
    pub current: SubPos,
}

/// Why a unit sprite sheet named by the definitions was rejected (cross validation).
#[derive(Debug)]
pub struct SpriteError {
    /// The units file.
    pub file: PathBuf,
    /// The definition's `sprite` field, e.g. `units[1].sprite`.
    pub field: String,
    pub image: TilesetImageError,
}

impl fmt::Display for SpriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "data file {}: {}: sprite image {}: {}: ",
            self.file.display(),
            self.field,
            self.image.file.display(),
            self.image.field,
        )?;
        match &self.image.kind {
            // The image check speaks of the size "the map declares" for tileset images; a sprite
            // sheet's size is fixed by the game.
            TilesetImageErrorKind::SizeMismatch { width, height, .. } => {
                let (expected_width, expected_height) = sheet_size();
                write!(
                    f,
                    "the image is {width}x{height} px, but a sprite sheet is \
                     {expected_width}x{expected_height} px"
                )
            }
            kind => kind.fmt(f),
        }
    }
}

impl std::error::Error for SpriteError {}

/// A group of starting units for which the map has too few free cells: fewer than `required`
/// passable, unoccupied cells are reachable from `anchor` (none if it lies outside the map).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartError {
    pub owner: PlayerId,
    pub unit_type: String,
    pub anchor: CellPos,
    pub found: usize,
    pub required: usize,
}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "starting units of player {}: {} {:?} units are placed around cell ({}, {}), but \
             only {} free cells are reachable there",
            self.owner.0, self.required, self.unit_type, self.anchor.x, self.anchor.y, self.found
        )?;
        if self.found == 0 {
            write!(f, " (the cell is outside the map or walled in)")?;
        }
        Ok(())
    }
}

impl std::error::Error for StartError {}

/// The cell of every starting unit: for each group in order, the free cells nearest to its
/// anchor (`sim::destination_cells`), skipping cells occupied in `grid` and cells given to
/// earlier groups. A group that does not get all of its cells is an error, so match setup never
/// spawns fewer units than configured (M2 review 1, P1-2).
pub fn plan_starting_units(
    grid: &Grid,
    groups: &[StartingGroup],
) -> Result<Vec<(PlayerId, &'static str, CellPos)>, StartError> {
    let mut planned = Vec::new();
    let mut taken = BTreeSet::new();
    for &(owner, unit_type, (x, y), count) in groups {
        let anchor = CellPos::new(x, y);
        let cells = destination_cells(grid, anchor, count, |cell| {
            grid.occupant(cell).is_some() || taken.contains(&cell)
        });
        if cells.len() < count {
            return Err(StartError {
                owner,
                unit_type: unit_type.to_owned(),
                anchor,
                found: cells.len(),
                required: count,
            });
        }
        for cell in cells {
            taken.insert(cell);
            planned.push((owner, unit_type, cell));
        }
    }
    Ok(planned)
}

/// Why the units could not be set up.
#[derive(Debug)]
pub enum SetupError {
    Data(DataError),
    Sprite(Box<SpriteError>),
}

impl fmt::Display for SetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SetupError::Data(error) => error.fmt(f),
            SetupError::Sprite(error) => error.fmt(f),
        }
    }
}

/// Size of a sprite sheet in pixels.
pub const fn sheet_size() -> (u32, u32) {
    (FRAME_SIZE * FACINGS, FRAME_SIZE)
}

/// Path of a sprite sheet relative to the asset directory.
pub fn sprite_asset_path(sprite: &str) -> PathBuf {
    Path::new(SPRITE_DIR).join(format!("{sprite}.png"))
}

/// A sprite sheet that passed its check.
#[derive(Debug)]
pub struct CheckedSheet {
    pub kind: UnitKind,
    /// Path relative to the asset directory.
    pub asset_path: PathBuf,
    /// The checked bytes, the only bytes the renderer may be given.
    pub bytes: Vec<u8>,
}

/// The unit definitions and the checked sprite sheet of every unit type.
#[derive(Debug)]
pub struct LoadedUnits {
    pub definitions: Definitions,
    pub sheets: Vec<CheckedSheet>,
}

/// Loads the unit definitions at `asset_root`/[`UNITS_PATH`] and checks the sprite sheet of
/// every definition.
pub fn load_units(asset_root: &Path) -> Result<LoadedUnits, SetupError> {
    let units_file = asset_root.join(UNITS_PATH);
    let definitions = sim::load_unit_definitions(&units_file).map_err(SetupError::Data)?;
    let (width, height) = sheet_size();
    let mut sheets = Vec::new();
    for (index, unit) in definitions.units().iter().enumerate() {
        let asset_path = sprite_asset_path(&unit.sprite);
        let bytes =
            check_png_file(&asset_root.join(&asset_path), width, height).map_err(|image| {
                SetupError::Sprite(Box::new(SpriteError {
                    file: units_file.clone(),
                    field: format!("units[{index}].sprite"),
                    image,
                }))
            })?;
        let kind = UnitKind(u16::try_from(index).unwrap_or(u16::MAX));
        sheets.push(CheckedSheet {
            kind,
            asset_path,
            bytes,
        });
    }
    Ok(LoadedUnits {
        definitions,
        sheets,
    })
}

/// Match setup: installs the definitions, registers the checked sprite sheets, and spawns the
/// starting units. Runs after the map is loaded; does nothing if the map failed.
fn setup_units(world: &mut World) {
    if !world.contains_resource::<MapView>() {
        return;
    }
    let root = FileAssetReader::new(AssetPlugin::default().file_path)
        .root_path()
        .clone();
    let LoadedUnits {
        definitions,
        sheets,
    } = match load_units(&root) {
        Ok(loaded) => loaded,
        Err(error) => {
            error!("{error}");
            world.write_message(AppExit::error());
            return;
        }
    };
    let starting = match plan_starting_units(world.resource::<Grid>(), &STARTING_UNITS) {
        Ok(starting) => starting,
        Err(error) => {
            error!("{error}");
            world.write_message(AppExit::error());
            return;
        }
    };
    let checked = world.resource::<CheckedAssets>().clone();
    let server = world.resource::<AssetServer>().clone();
    let mut sprites = UnitSprites::default();
    for sheet in sheets {
        checked.insert(sheet.asset_path.clone(), sheet.bytes);
        sprites.0.insert(sheet.kind, server.load(sheet.asset_path));
    }
    world.insert_resource(sprites);
    world.insert_resource(definitions);

    let mut spawned = 0;
    for (owner, unit_type, cell) in starting {
        match sim::spawn_unit(world, owner, unit_type, cell) {
            Ok(_) => spawned += 1,
            Err(error) => {
                error!(
                    "starting unit {unit_type:?} at ({}, {}): {error}",
                    cell.x, cell.y
                );
                world.write_message(AppExit::error());
                return;
            }
        }
    }
    info!("{spawned} units spawned");
    world.resource_mut::<UnitStatus>().spawned = spawned;
}

/// Colour that tints the light grey sprites of a player's units.
pub fn owner_colour(owner: PlayerId) -> Color {
    if owner == HUMAN {
        Color::srgb(0.55, 0.75, 1.0)
    } else {
        Color::srgb(1.0, 0.5, 0.45)
    }
}

/// The region of the sprite sheet that shows `facing`.
pub fn facing_rect(facing: Direction) -> Rect {
    let left = f32::from(facing.index()) * FRAME_SIZE as f32;
    Rect::new(left, 0.0, left + FRAME_SIZE as f32, FRAME_SIZE as f32)
}

/// The fine position between `previous` and `current` at `fraction` (0 to 1) of a tick, in
/// continuous cell coordinates (subcells / 256).
pub fn interpolate(previous: SubPos, current: SubPos, fraction: f32) -> Vec2 {
    let fraction = fraction.clamp(0.0, 1.0);
    let lerp = |a: i32, b: i32| (a as f32 + (b - a) as f32 * fraction) / SUBCELLS_PER_CELL as f32;
    Vec2::new(lerp(previous.x, current.x), lerp(previous.y, current.y))
}

/// World position of the ground point of a unit at continuous cell coordinates `cell`.
pub fn ground_point(cell: Vec2, tile: TileSize) -> Vec2 {
    to_world(cell.x, cell.y, tile)
}

/// Draw order: units lower on the screen (larger u + v) are drawn in front.
pub fn unit_depth(cell: Vec2) -> f32 {
    UNIT_Z + (cell.x + cell.y) / 1024.0
}

/// Creates a view for every simulation unit that has none.
fn spawn_unit_views(
    mut commands: Commands,
    mut views: ResMut<UnitViews>,
    sprites: Option<Res<UnitSprites>>,
    units: Query<(Entity, &SimId, &Owner, &Unit, &Position, &Facing)>,
) {
    let Some(sprites) = sprites else {
        return;
    };
    for (unit, id, owner, kind, position, facing) in &units {
        if views.0.contains_key(id) {
            continue;
        }
        let Some(image) = sprites.0.get(&kind.kind) else {
            continue;
        };
        let view = commands
            .spawn((
                UnitView {
                    unit,
                    id: *id,
                    owner: owner.0,
                    previous: position.0,
                    current: position.0,
                },
                Sprite {
                    image: image.clone(),
                    rect: Some(facing_rect(facing.0)),
                    color: owner_colour(owner.0),
                    ..default()
                },
                Transform::default(),
            ))
            .id();
        views.0.insert(*id, view);
    }
}

/// Records every unit's position before a simulation tick (runs in `FixedUpdate` right before
/// the tick).
pub fn record_previous_positions(mut views: Query<&mut UnitView>, positions: Query<&Position>) {
    for mut view in &mut views {
        if let Ok(position) = positions.get(view.unit) {
            view.previous = position.0;
        }
    }
}

/// Places every view between its unit's previous and current position, and shows its facing.
fn place_unit_views(
    map: Option<Res<MapView>>,
    time: Res<Time<Fixed>>,
    mut commands: Commands,
    mut registry: ResMut<UnitViews>,
    mut views: Query<(Entity, &mut UnitView, &mut Sprite, &mut Transform)>,
    units: Query<(&Position, &Facing)>,
) {
    let Some(map) = map else {
        return;
    };
    let fraction = time.overstep_fraction();
    for (entity, mut view, mut sprite, mut transform) in &mut views {
        let Ok((position, facing)) = units.get(view.unit) else {
            // The unit no longer exists (units die from M4 on).
            registry.0.remove(&view.id);
            commands.entity(entity).despawn();
            continue;
        };
        view.current = position.0;
        let cell = interpolate(view.previous, view.current, fraction);
        let ground = ground_point(cell, map.tile);
        transform.translation = Vec3::new(ground.x, ground.y + GROUND_OFFSET_PX, unit_depth(cell));
        let rect = facing_rect(facing.0);
        if sprite.rect != Some(rect) {
            sprite.rect = Some(rect);
        }
    }
}

/// Logs the simulation's events. The simulation advances its event buffer once per tick, so a
/// reader sees the events of the last two ticks (`sim::SimEvent`); each event is read once.
/// Events of earlier ticks are missed if more than two ticks run between two frames, which
/// only affects this log.
fn log_sim_events(mut events: MessageReader<SimEvent>) {
    for event in events.read() {
        match event {
            SimEvent::CommandRejected {
                player,
                command,
                reason,
            } => warn!(
                "command of player {} rejected: {reason}: {command:?}",
                player.0
            ),
            SimEvent::MoveCompleted { unit, retargeted } => {
                if *retargeted {
                    debug!("unit {} stopped near its destination", unit.0);
                } else {
                    debug!("unit {} arrived", unit.0);
                }
            }
            SimEvent::MoveFailed { unit, reason } => {
                info!("unit {} stopped: {reason:?}", unit.0);
            }
        }
    }
}

/// The sprite generator, to tie the sprite layout to it in the tests.
#[cfg(test)]
#[path = "../examples/gen_unit_sprites.rs"]
#[allow(dead_code)]
mod gen_unit_sprites;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sprite_layout_matches_the_generator() {
        assert_eq!(FRAME_SIZE, gen_unit_sprites::FRAME);
        assert_eq!(FACINGS, gen_unit_sprites::FACINGS);
        // The sprite's centre is drawn `GROUND_OFFSET_PX` above the unit's ground point, where
        // the generator puts the ground point under the unit.
        let (x, y) = gen_unit_sprites::GROUND;
        assert_eq!(x, FRAME_SIZE / 2);
        assert_eq!(y as f32 - FRAME_SIZE as f32 / 2.0, GROUND_OFFSET_PX);
        // Every production sprite has a generator entry, so its placeholder is reproducible.
        let definitions = load_units(&assets_root())
            .unwrap_or_else(|error| panic!("{error}"))
            .definitions;
        for unit in definitions.units() {
            assert!(
                gen_unit_sprites::SPRITES
                    .iter()
                    .any(|kind| kind.name == unit.sprite),
                "{}",
                unit.sprite
            );
        }
    }

    fn assets_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    #[test]
    fn the_production_units_load_and_every_sprite_passes_its_check() {
        // Cross validation of the production data (PLAN.md section 5): every sprite named by
        // `assets/data/units.ron` exists and is inside the PNG subset.
        let LoadedUnits {
            definitions,
            sheets,
        } = load_units(&assets_root()).unwrap_or_else(|error| panic!("{error}"));
        assert!(!definitions.units().is_empty());
        assert_eq!(sheets.len(), definitions.units().len());
        for (sheet, unit) in sheets.iter().zip(definitions.units()) {
            assert_eq!(definitions.unit(sheet.kind), Some(unit));
            assert_eq!(sheet.asset_path, sprite_asset_path(&unit.sprite));
            let file = std::fs::read(assets_root().join(&sheet.asset_path)).expect("sprite file");
            assert_eq!(sheet.bytes, file);
        }
        // The starting units name defined unit types.
        for (_, unit_type, _, count) in STARTING_UNITS {
            assert!(definitions.unit_kind(unit_type).is_some(), "{unit_type}");
            assert!(count > 0);
        }
    }

    fn fixture_grid(name: &str) -> Grid {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../sim/tests/fixtures/maps")
            .join(format!("{name}.tmx"));
        sim::load_map(&path)
            .unwrap_or_else(|error| panic!("{error}"))
            .grid
    }

    #[test]
    fn the_demo_map_holds_every_starting_unit_on_its_own_free_cell() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join(crate::map::MAP_PATH);
        let grid = sim::load_map(&path)
            .unwrap_or_else(|error| panic!("{error}"))
            .grid;
        let planned = plan_starting_units(&grid, &STARTING_UNITS).expect("demo map");
        assert_eq!(planned.len(), starting_unit_total());
        let cells: BTreeSet<CellPos> = planned.iter().map(|&(.., cell)| cell).collect();
        assert_eq!(cells.len(), planned.len(), "cells are distinct");
        assert!(cells.iter().all(|&cell| grid.is_passable(cell)));
        for (owner, unit_type, _, count) in STARTING_UNITS {
            let placed = planned
                .iter()
                .filter(|&&(o, t, _)| o == owner && t == unit_type)
                .count();
            assert_eq!(placed, count, "{unit_type}");
        }
    }

    #[test]
    fn starting_units_that_do_not_fit_are_a_setup_error() {
        // M2 review 1, P1-2: the arena fixture is 32x32, so the opponent's anchors of the demo
        // match lie outside it; before, those 8 units and the 6 scouts were silently dropped.
        let arena = fixture_grid("arena_32x32");
        assert_eq!(
            plan_starting_units(&arena, &STARTING_UNITS),
            Err(StartError {
                owner: HUMAN,
                unit_type: "scout".into(),
                anchor: CellPos::new(14, 34),
                found: 0,
                required: 6,
            })
        );
        let message = plan_starting_units(&arena, &[(OPPONENT, "scout", (-1, 3), 2)])
            .expect_err("outside")
            .to_string();
        assert_eq!(
            message,
            "starting units of player 1: 2 \"scout\" units are placed around cell (-1, 3), but \
             only 0 free cells are reachable there (the cell is outside the map or walled in)"
        );
        // An anchor with too few free cells reachable from it: a pocket of three cells in rock.
        let mut grid = fixture_grid("open_16x16");
        let rock = sim::Cell {
            passable: false,
            buildable: false,
            ore: 0,
        };
        for x in 3..=7 {
            for y in 3..=5 {
                if !(y == 4 && (4..=6).contains(&x)) {
                    grid.set_terrain(CellPos::new(x, y), rock);
                }
            }
        }
        let pocket = [(HUMAN, "light_tank", (5, 4), 3)];
        assert_eq!(plan_starting_units(&grid, &pocket).map(|p| p.len()), Ok(3));
        assert_eq!(
            plan_starting_units(&grid, &[(HUMAN, "light_tank", (5, 4), 4)]),
            Err(StartError {
                owner: HUMAN,
                unit_type: "light_tank".into(),
                anchor: CellPos::new(5, 4),
                found: 3,
                required: 4,
            })
        );
        // Cells given to an earlier group are not free for a later one.
        assert_eq!(
            plan_starting_units(
                &grid,
                &[
                    (HUMAN, "light_tank", (5, 4), 2),
                    (OPPONENT, "scout", (4, 4), 2)
                ]
            ),
            Err(StartError {
                owner: OPPONENT,
                unit_type: "scout".into(),
                anchor: CellPos::new(4, 4),
                found: 1,
                required: 2,
            })
        );
    }

    #[test]
    fn a_missing_or_malformed_sprite_is_reported_with_the_data_file_and_field() {
        let dir =
            std::env::temp_dir().join(format!("mygame001_game_units_tests_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("data")).expect("data directory");
        std::fs::create_dir_all(dir.join(SPRITE_DIR)).expect("sprite directory");
        std::fs::write(
            dir.join(UNITS_PATH),
            "(schema_version: 1, units: [(id: \"a\", speed: 5, sprite: \"one\"), \
             (id: \"b\", speed: 5, sprite: \"two\")])",
        )
        .expect("units file");
        let production = assets_root().join(SPRITE_DIR).join("light_tank.png");
        std::fs::copy(&production, dir.join(SPRITE_DIR).join("one.png")).expect("sprite");
        let message = load_units(&dir)
            .expect_err("two.png is missing")
            .to_string();
        assert!(message.contains("units[1].sprite"), "{message}");
        assert!(
            message.contains(&dir.join(UNITS_PATH).display().to_string()),
            "{message}"
        );
        assert!(message.contains("two.png"), "{message}");
        assert!(message.contains("cannot be read"), "{message}");
        // A sheet of the wrong size is rejected as well.
        let tileset = assets_root().join("maps/demo/demo_tiles.png");
        std::fs::copy(&tileset, dir.join(SPRITE_DIR).join("two.png")).expect("sprite");
        let message = load_units(&dir).expect_err("wrong size").to_string();
        assert!(message.contains("units[1].sprite"), "{message}");
        assert!(
            message.contains("the image is 512x32 px, but a sprite sheet is 512x64 px"),
            "{message}"
        );
        // A malformed units file is reported by the data loader.
        std::fs::write(dir.join(UNITS_PATH), "(schema_version: 2)").expect("units file");
        let message = load_units(&dir).expect_err("version").to_string();
        assert!(
            message.contains("schema version 2 is not supported"),
            "{message}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_facing_has_its_own_frame() {
        let mut lefts: Vec<f32> = Direction::ALL
            .into_iter()
            .map(|facing| facing_rect(facing).min.x)
            .collect();
        assert_eq!(lefts.len(), FACINGS as usize);
        lefts.dedup();
        assert_eq!(lefts.len(), FACINGS as usize);
        for facing in Direction::ALL {
            let rect = facing_rect(facing);
            assert_eq!(rect.size(), Vec2::splat(FRAME_SIZE as f32));
            assert!(rect.max.x <= sheet_size().0 as f32);
        }
    }

    #[test]
    fn interpolation_moves_from_the_previous_to_the_current_position() {
        let previous = SubPos::new(128, 128);
        let current = SubPos::new(160, 96);
        assert_eq!(interpolate(previous, current, 0.0), Vec2::new(0.5, 0.5));
        assert_eq!(interpolate(previous, current, 1.0), Vec2::new(0.625, 0.375));
        assert_eq!(
            interpolate(previous, current, 0.5),
            Vec2::new(0.5625, 0.4375)
        );
        // Out-of-range fractions are clamped.
        assert_eq!(interpolate(previous, current, 7.0), Vec2::new(0.625, 0.375));
        // A unit at a cell centre is drawn at the centre of that cell.
        let tile = TileSize::new(64, 32);
        let centre = SubPos::centre_of(CellPos::new(3, 5));
        assert_eq!(
            ground_point(interpolate(centre, centre, 0.3), tile),
            crate::projection::cell_centre(CellPos::new(3, 5), tile)
        );
    }

    #[test]
    fn units_lower_on_the_screen_are_drawn_in_front() {
        let far = unit_depth(Vec2::new(10.0, 10.0));
        let near = unit_depth(Vec2::new(10.0, 11.0));
        assert!(near > far);
        // Every unit of the largest map is drawn above the map (z = 0) and inside the camera's
        // depth range.
        let max = sim::MAX_MAP_SIDE as f32;
        assert!(unit_depth(Vec2::ZERO) > 0.0);
        assert!(unit_depth(Vec2::new(max, max)) < 2.0);
    }
}
