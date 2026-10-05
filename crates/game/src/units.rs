//! Units in the game (PLAN.md section 7, M2): match setup from the unit definitions, the
//! placeholder sprite sheets, and drawing the simulation's units with eight facings and
//! interpolated positions.
//!
//! The game reads simulation state and never writes it (PLAN.md section 4, invariant I5): each
//! simulation unit is drawn by a separate view entity that copies what it needs from the unit's
//! components, read through `crate::sim_host::SimHost`. The only writes into the simulation world
//! are match setup before the first tick (`sim::setup_match`, with the starting units of the
//! skirmish file, D11, D40) and the commands of the interface, both in `crate::sim_host`
//! (docs/DECISIONS.md D34).
//!
//! The unit definitions are loaded by `sim::load_unit_definitions` (the RON subset of D18). Each
//! definition names a sprite sheet, which must exist and pass the PNG subset check before the
//! match starts (cross validation, PLAN.md section 5; D20); the renderer receives exactly the
//! checked bytes (`crate::checked_assets`).

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use sim::{
    DataError, Definitions, Direction, Facing, Mobile, Owner, PlayerId, Players, Position,
    SUBCELLS_PER_CELL, SimEvent, SimId, SubPos, Unit, UnitKind,
};

use crate::buildings::{
    BuildingSpriteError, CheckedBuildingSprite, ECONOMY_PATH, check_building_sprites,
};
use crate::map::MapView;
use crate::projection::{TileSize, to_world};
use crate::sim_host::SimHost;
use crate::tileset_image::{TilesetImageError, TilesetImageErrorKind, check_png_file};

/// The units file, relative to the asset directory.
pub const UNITS_PATH: &str = "data/units.ron";

/// The combat file, relative to the asset directory (M4).
pub const COMBAT_PATH: &str = "data/combat.ron";

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

/// The other player: the computer player of a skirmish match (M6).
pub const OPPONENT: PlayerId = PlayerId(1);

/// Lowest z of a unit sprite. The map's layers lie at z = 0 (`bevy_ecs_tiled`).
const UNIT_Z: f32 = 1.0;

pub struct UnitsPlugin;

impl Plugin for UnitsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UnitViews>()
            .init_resource::<UnitStatus>()
            .add_systems(Update, (spawn_unit_views, log_sim_events))
            .add_systems(
                PostUpdate,
                place_unit_views.before(bevy::transform::TransformSystems::Propagate),
            );
    }
}

/// The sprite sheet of each unit type.
#[derive(Resource, Debug, Default)]
pub struct UnitSprites(
    pub BTreeMap<UnitKind, Handle<Image>>,
    pub BTreeMap<UnitKind, Handle<Image>>,
);

/// The view entity of each simulation unit.
#[derive(Resource, Debug, Default)]
pub struct UnitViews(pub BTreeMap<SimId, Entity>);

/// What has happened to the units of the current match so far; read by the smoke test.
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

/// Why the units could not be set up.
#[derive(Debug)]
pub enum SetupError {
    Data(DataError),
    Sprite(Box<SpriteError>),
    BuildingSprite(Box<BuildingSpriteError>),
}

impl fmt::Display for SetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SetupError::Data(error) => error.fmt(f),
            SetupError::Sprite(error) => error.fmt(f),
            SetupError::BuildingSprite(error) => error.fmt(f),
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

/// Loads the unit definitions with the economy file (`crate::buildings::ECONOMY_PATH`) and the
/// combat file ([`COMBAT_PATH`]), and checks the sprite of every unit and building type.
pub fn load_match_data(
    asset_root: &Path,
) -> Result<(LoadedUnits, Vec<CheckedBuildingSprite>), SetupError> {
    let LoadedUnits {
        definitions,
        sheets,
    } = load_units(asset_root)?;
    let economy_file = asset_root.join(ECONOMY_PATH);
    let definitions = definitions
        .with_economy(&economy_file)
        .and_then(|definitions| definitions.with_combat(asset_root.join(COMBAT_PATH)))
        .map_err(SetupError::Data)?;
    let buildings = check_building_sprites(asset_root, &economy_file, &definitions)
        .map_err(SetupError::BuildingSprite)?;
    Ok((
        LoadedUnits {
            definitions,
            sheets,
        },
        buildings,
    ))
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
pub(crate) fn spawn_unit_views(
    mut commands: Commands,
    mut views: ResMut<UnitViews>,
    sprites: Option<Res<UnitSprites>>,
    host: Res<SimHost>,
) {
    let Some(sprites) = sprites else {
        return;
    };
    let Some(mut units) = host
        .world()
        .try_query::<(Entity, &SimId, &Owner, &Unit, &Position, &Facing)>()
    else {
        return;
    };
    for (unit, id, owner, kind, position, facing) in units.iter(host.world()) {
        if views.0.contains_key(id) {
            continue;
        }
        let team = if owner.0 == HUMAN {
            &sprites.0
        } else {
            &sprites.1
        };
        let Some(image) = team.get(&kind.kind) else {
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
                    color: Color::WHITE,
                    ..default()
                },
                Transform::default(),
                // Another player's unit is shown only once `place_unit_views` finds it visible.
                if owner.0 == HUMAN {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                },
            ))
            .id();
        views.0.insert(*id, view);
    }
}

/// Records every unit's position before a simulation tick (runs in `FixedUpdate` right before
/// the tick).
pub fn record_previous_positions(mut views: Query<&mut UnitView>, host: Res<SimHost>) {
    for mut view in &mut views {
        if let Some(position) = host.world().get::<Position>(view.unit) {
            view.previous = position.0;
        }
    }
}

/// Whether the human player is shown a unit of `owner` that moves as `mobile`: its own units
/// always, other players' units only while the human player sees them (M4, hiding of unseen
/// enemies; docs/DECISIONS.md D31). Before the first tick, and without a vision, nothing of
/// another player is shown.
pub fn unit_shown(owner: PlayerId, mobile: &Mobile, players: &Players) -> bool {
    owner == HUMAN
        || players
            .get(HUMAN)
            .is_some_and(|state| state.vision().sees_unit(mobile))
}

/// Places every view between its unit's previous and current position, shows its facing, and
/// hides it while the human player does not see it.
#[allow(clippy::type_complexity)]
fn place_unit_views(
    map: Option<Res<MapView>>,
    time: Res<Time<Fixed>>,
    host: Res<SimHost>,
    mut commands: Commands,
    mut registry: ResMut<UnitViews>,
    // Disjoint from the building views' query (`crate::buildings`), which also writes
    // `Visibility`.
    mut views: Query<
        (
            Entity,
            &mut UnitView,
            &mut Sprite,
            &mut Transform,
            &mut Visibility,
        ),
        Without<crate::buildings::BuildingView>,
    >,
) {
    let (Some(map), Some(players)) = (map, host.players()) else {
        return;
    };
    let world = host.world();
    let fraction = time.overstep_fraction();
    for (entity, mut view, mut sprite, mut transform, mut visibility) in &mut views {
        let (Some(position), Some(facing), Some(mobile)) = (
            world.get::<Position>(view.unit),
            world.get::<Facing>(view.unit),
            world.get::<Mobile>(view.unit),
        ) else {
            // The unit no longer exists (destroyed, M4).
            registry.0.remove(&view.id);
            commands.entity(entity).despawn();
            continue;
        };
        let shown = if unit_shown(view.owner, mobile, players) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != shown {
            *visibility = shown;
        }
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

/// Logs the simulation's events. `crate::sim_host` copies every tick's events into the game's
/// message buffer, which Bevy advances once per frame, so each event is read once, also when
/// several ticks run in one frame (M5; before, events of more than two ticks per frame were
/// missed, M2 implementation debt item 2).
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
            SimEvent::ProductionComplete { player, item } => {
                info!("player {}: {item:?} is complete", player.0);
            }
            SimEvent::ProductionCancelled {
                player,
                item,
                refund,
            } => info!(
                "player {}: {item:?} cancelled, {refund} credits refunded",
                player.0
            ),
            SimEvent::UnitSpawned {
                player,
                unit,
                building,
            } => info!(
                "player {}: unit {} left building {}",
                player.0, unit.0, building.0
            ),
            SimEvent::BuildingPlaced {
                player,
                building,
                kind,
            } => info!(
                "player {}: building {} of type {} placed",
                player.0, building.0, kind.0
            ),
            SimEvent::BuildingSold {
                player,
                building,
                refund,
            } => info!(
                "player {}: building {} sold for {refund} credits",
                player.0, building.0
            ),
            SimEvent::OreDelivered {
                player,
                harvester,
                ore,
                credits,
                ..
            } => debug!(
                "player {}: harvester {} delivered {ore} ore for {credits} credits",
                player.0, harvester.0
            ),
            SimEvent::HarvesterIdle { unit, reason } => {
                info!("harvester {} stopped: {reason:?}", unit.0);
            }
            SimEvent::WeaponFired {
                attacker, target, ..
            } => debug!("{} fired at {}", attacker.0, target.0),
            SimEvent::ProjectileExpired {
                attacker, target, ..
            } => debug!(
                "a projectile of {} found its target {} gone",
                attacker.0, target.0
            ),
            SimEvent::UnitDestroyed { player, unit } => {
                info!("player {}: unit {} destroyed", player.0, unit.0);
            }
            SimEvent::BuildingDestroyed {
                player,
                building,
                lost_units,
                ..
            } => info!(
                "player {}: building {} destroyed, {} waiting units lost with it",
                player.0,
                building.0,
                lost_units.len()
            ),
            SimEvent::AttackEnded {
                unit,
                target,
                reason,
            } => info!("unit {} stopped attacking {}: {reason:?}", unit.0, target.0),
            SimEvent::PlayerDefeated { player } => {
                info!("player {} is defeated: it owns no building", player.0);
            }
            SimEvent::MatchEnded { winner } => match winner {
                Some(winner) => info!("the match is over: player {} wins", winner.0),
                None => info!("the match is over: a draw"),
            },
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
    use std::collections::BTreeSet;

    use sim::{CellPos, Grid, StartError, plan_starting_units};

    use super::*;

    /// The starting units of the demo match until M5 (game code until M6, docs/DECISIONS.md D20
    /// item 4), kept as test input for `plan_starting_units`, which moved to `sim` in M6.
    const DEMO_GROUPS: [(PlayerId, &str, (i32, i32), usize); 4] = [
        (HUMAN, "light_tank", (10, 28), 12),
        (HUMAN, "scout", (14, 34), 6),
        (OPPONENT, "light_tank", (50, 30), 5),
        (OPPONENT, "scout", (52, 36), 3),
    ];

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
        // The starting units of every faction of the skirmish file (M6: data, D40) name defined
        // unit types, in groups of at least one.
        let catalogue = crate::skirmish::tests::production_catalogue();
        for faction in &catalogue.skirmish.factions {
            for group in &faction.units {
                assert_eq!(
                    definitions.unit(group.kind),
                    catalogue.definitions.unit(group.kind)
                );
                assert!(definitions.unit(group.kind).is_some(), "{group:?}");
                assert!(group.count > 0);
            }
        }
    }

    #[test]
    fn the_production_data_loads_with_the_combat_file() {
        // Cross validation of the production data (PLAN.md section 5): the combat file gives
        // every unit and building type of the production files hit points, armour, and sight.
        let (LoadedUnits { definitions, .. }, buildings) =
            load_match_data(&assets_root()).unwrap_or_else(|error| panic!("{error}"));
        assert!(definitions.has_combat());
        for index in 0..definitions.units().len() {
            let kind = UnitKind(u16::try_from(index).expect("index"));
            assert!(definitions.unit_combat(kind).is_some(), "{index}");
        }
        assert_eq!(buildings.len(), definitions.buildings().len());
        for index in 0..definitions.buildings().len() {
            let kind = sim::BuildingKind(u16::try_from(index).expect("index"));
            assert!(definitions.building_combat(kind).is_some(), "{index}");
        }
        // The demo match has a defensive structure: a building type with a weapon.
        let turret = definitions.building_kind("gun_turret").expect("turret");
        assert!(
            definitions
                .building_combat(turret)
                .and_then(|terms| terms.weapon)
                .is_some()
        );
        // The computer player of the demo match builds it (M6: the starting buildings are data
        // of the skirmish file, and the computer player's buildings follow the AI file's build
        // order, D40, D42).
        let catalogue = crate::skirmish::tests::production_catalogue();
        assert!(catalogue.ai.build_order.contains(&turret));
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
        // M6: the starting units are those of the skirmish file's factions at the map's start
        // positions, placed by `sim::setup_match` (D40). Every one of them gets its own passable
        // cell, none on a building, and every group is complete.
        let catalogue = crate::skirmish::tests::production_catalogue();
        let selection = crate::skirmish::MenuSelection::initial(&catalogue);
        let host = crate::sim_host::tests::demo_host();
        let world = host.world();
        let grid = host.grid().expect("grid");
        let mut units = world
            .try_query::<(&Owner, &Unit, &Mobile)>()
            .expect("registered");
        let mut cells = BTreeSet::new();
        let mut placed: BTreeMap<(PlayerId, UnitKind), u32> = BTreeMap::new();
        for (owner, unit, mobile) in units.iter(world) {
            assert!(cells.insert(mobile.cell), "cells are distinct");
            assert!(grid.is_passable(mobile.cell));
            assert_eq!(grid.building(mobile.cell), None);
            *placed.entry((owner.0, unit.kind)).or_insert(0) += 1;
        }
        let mut expected: BTreeMap<(PlayerId, UnitKind), u32> = BTreeMap::new();
        for setup in selection.players() {
            for group in &catalogue.skirmish.factions[setup.faction].units {
                *expected.entry((setup.player, group.kind)).or_insert(0) += group.count;
            }
        }
        assert!(!expected.is_empty());
        assert_eq!(placed, expected);
        // The M2 demo groups still fit the demo map, each unit on its own passable cell.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join(crate::map::MAP_PATH);
        let demo: Grid = sim::load_map(&path)
            .unwrap_or_else(|error| panic!("{error}"))
            .grid;
        let planned = plan_starting_units(&demo, &DEMO_GROUPS).expect("demo map");
        assert_eq!(planned.len(), 12 + 6 + 5 + 3);
        let cells: BTreeSet<CellPos> = planned.iter().map(|&(.., cell)| cell).collect();
        assert_eq!(cells.len(), planned.len(), "cells are distinct");
        assert!(cells.iter().all(|&cell| demo.is_passable(cell)));
        for (owner, unit_type, _, count) in DEMO_GROUPS {
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
            plan_starting_units(&arena, &DEMO_GROUPS),
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
