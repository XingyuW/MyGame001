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

/// Original procedural foliage; hidden Mirage identities never gain a sprite, label or selection target.
fn draw_mirage_trees(host: Res<SimHost>, map: Option<Res<MapView>>, mut gizmos: Gizmos) {
    let Some(map) = map else {
        return;
    };
    let world = host.world();
    let Some(state) = world.get_resource::<sim::SpecialState>() else {
        return;
    };
    let Some(players) = host.players() else {
        return;
    };
    for e in world.iter_entities() {
        let (Some(id), Some(m)) = (e.get::<SimId>(), e.get::<Mobile>()) else {
            continue;
        };
        let Some(&v) = state.mirage_trees.get(id) else {
            continue;
        };
        if !state.hidden.contains(&(HUMAN, *id))
            || !players.get(HUMAN).is_some_and(|p| p.vision().sees_unit(m))
        {
            continue;
        }
        let p = ground_point(
            Vec2::new(m.cell.x as f32 + 0.5, m.cell.y as f32 + 0.5),
            map.tile,
        );
        gizmos.line_2d(p, p + Vec2::Y * 25.0, Color::srgb(0.30, 0.19, 0.09));
        let green = Color::srgb(0.12 + f32::from(v) * 0.025, 0.32, 0.13);
        for (offset, r) in [
            (Vec2::new(-6.0, 23.0), 9.0),
            (Vec2::new(6.0, 25.0), 10.0),
            (Vec2::new(0.0, 33.0), 11.0),
        ] {
            gizmos.circle_2d(p + offset, r + f32::from(v), green);
        }
    }
}

#[cfg(test)]
use sim::Direction;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use sim::{
    DataError, Definitions, Facing, Mobile, Owner, PlayerId, Players, Position, SUBCELLS_PER_CELL,
    SimEvent, SimId, SubPos, Unit, UnitKind,
};

use crate::buildings::{
    BuildingSpriteError, CheckedBuildingSprite, ECONOMY_PATH, check_building_sprites,
};
use crate::hd_art::UNIT_GROUND_OFFSET;
use crate::map::MapView;
use crate::projection::{TileSize, to_world};
use crate::sim_host::SimHost;
use crate::tileset_image::{TilesetImageError, TilesetImageErrorKind, check_png_file};

/// The units file, relative to the asset directory.
pub const UNITS_PATH: &str = "data/units.ron";

/// The combat file, relative to the asset directory (M4).
pub const COMBAT_PATH: &str = "data/combat.ron";

/// The rules file, relative to the asset directory (RA1).
pub const RULES_PATH: &str = "data/rules.ron";

/// The arsenal file, relative to the asset directory.
pub const ARSENAL_PATH: &str = "data/arsenal.ron";

/// Directory of the data files, relative to the asset directory.
pub const DATA_DIR: &str = "data";

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
#[cfg(test)]
pub const OPPONENT: PlayerId = PlayerId(1);

/// Lowest z of a unit sprite. The map's layers lie at z = 0 (`bevy_ecs_tiled`).
const UNIT_Z: f32 = 1.0;

/// How high above the ground an aircraft flies, in world pixels: two cell heights, above the
/// roofs of most buildings.
pub const FLIGHT_ALTITUDE: f32 = 64.0;

/// How high flying infantry (jet packs) hovers.
pub const HOVER_ALTITUDE: f32 = 40.0;

/// How high a strike aircraft sits while it rearms on the ground beside its air base.
pub const LANDED_ALTITUDE: f32 = 3.0;

/// World pixels per second an aircraft climbs or descends.
const CLIMB_RATE: f32 = 48.0;

/// z of an aircraft's shadow: on the ground, under every unit and building.
const AIR_SHADOW_Z: f32 = 0.95;

/// How much darker the ground is under an aircraft at cruising height; nearer the ground the
/// shadow is darker and sharper.
const AIR_SHADOW_ALPHA: f32 = 0.5;

/// The shadow is the aircraft's silhouette laid on the ground: flattened to this share of its
/// height.
const AIR_SHADOW_SQUASH: f32 = 0.62;

/// How far above its frame's ground point an aircraft's body is drawn (its sprite has no baked
/// shadow, `scripts/render_hd_blender.py`), so that the silhouette's middle lies on the ground
/// point under the aircraft.
const AIR_BODY_HEIGHT: f32 = 14.0;

pub struct UnitsPlugin;

impl Plugin for UnitsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UnitViews>()
            .init_resource::<UnitStatus>()
            .init_resource::<Altitudes>()
            .add_systems(
                Update,
                (spawn_unit_views, log_sim_events, draw_mirage_trees),
            )
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

/// The walk cycle of each infantry type that has one (`sprites/hd/walk`): the human player's
/// copy and the first opponent's, as [`UnitSprites`].
#[derive(Resource, Debug, Default)]
pub struct WalkSprites(pub BTreeMap<UnitKind, (Handle<Image>, Handle<Image>)>);

/// The view entity of each simulation unit.
#[derive(Resource, Debug, Default)]
pub struct UnitViews(pub BTreeMap<SimId, Entity>);

/// How high above its ground point every unit of the air domain is drawn now, in world pixels
/// (units on the ground are not listed). Written by `place_unit_views`; whatever draws something
/// at a unit read from the simulation (shots and projectiles, `crate::fog`) raises it by this.
#[derive(Resource, Debug, Default)]
pub struct Altitudes(pub BTreeMap<SimId, f32>);

impl Altitudes {
    pub fn of(&self, id: SimId) -> f32 {
        self.0.get(&id).copied().unwrap_or(0.0)
    }
}

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
    /// How high above its ground point the view is drawn now (0 for units on the ground). An
    /// aircraft starts on the ground where it is built and climbs to its flying height.
    pub altitude: f32,
    /// Where it was drawn last (in cells), and how far through its walk cycle that has brought
    /// it (in cycles): a figure's legs move with the ground it covers.
    pub drawn: Option<Vec2>,
    pub stride: f32,
}

/// The shadow an aircraft casts on the ground far below it: a child of its unit view.
#[derive(Component, Debug)]
pub struct AirShadow;

/// The height an air unit flies at: `infantry` hovers lower than aircraft, and a strike
/// aircraft that is rearming at its base (`landed`) sits on the ground.
pub fn flight_altitude(infantry: bool, landed: bool) -> f32 {
    if landed {
        LANDED_ALTITUDE
    } else if infantry {
        HOVER_ALTITUDE
    } else {
        FLIGHT_ALTITUDE
    }
}

/// `altitude` after `seconds` of climbing or descending towards `target`.
pub fn approach_altitude(altitude: f32, target: f32, seconds: f32) -> f32 {
    let step = CLIMB_RATE * seconds.max(0.0);
    if (target - altitude).abs() <= step {
        target
    } else {
        altitude + step.copysign(target - altitude)
    }
}

/// Where an aircraft's shadow lies relative to its view (which is drawn `altitude` above the
/// ground), and how dark it is: straight below on the ground point, fainter the higher the
/// aircraft flies.
pub fn air_shadow(altitude: f32) -> (Vec2, f32) {
    let below =
        UNIT_GROUND_OFFSET * (1.0 - AIR_SHADOW_SQUASH) + AIR_BODY_HEIGHT * AIR_SHADOW_SQUASH;
    let height = (altitude / FLIGHT_ALTITUDE).clamp(0.0, 1.0);
    (
        Vec2::new(0.0, -altitude - below),
        AIR_SHADOW_ALPHA * (1.3 - 0.3 * height),
    )
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
    load_units_file(asset_root, asset_root.join(UNITS_PATH))
}

/// [`load_units`] of the units file `units_file`, with the sprites of `asset_root`.
fn load_units_file(asset_root: &Path, units_file: PathBuf) -> Result<LoadedUnits, SetupError> {
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

/// Loads the unit definitions with the economy file (`crate::buildings::ECONOMY_PATH`), the
/// combat file ([`COMBAT_PATH`]), the rules file ([`RULES_PATH`]) and the arsenal
/// ([`ARSENAL_PATH`]), and checks the sprite of every unit and building type.
pub fn load_match_data(
    asset_root: &Path,
) -> Result<(LoadedUnits, Vec<CheckedBuildingSprite>), SetupError> {
    let units = load_units(asset_root)?;
    with_match_data(units, asset_root, &asset_root.join(DATA_DIR), true)
}

/// The file of the data directory `data` that the production path `path` (a file of
/// [`DATA_DIR`]) names.
pub(crate) fn data_file(data: &Path, path: &str) -> PathBuf {
    data.join(Path::new(path).file_name().unwrap_or_default())
}

/// [`load_match_data`] of a ruleset whose data files lie in the directory `data`, under the file
/// names of the production files, with the sprites of `asset_root`; `rules` says whether it has
/// a rules file. For the tests of the legacy ruleset fixture (the data before RA1), which has
/// none: the game itself always loads one.
#[cfg(test)]
pub(crate) fn load_ruleset_data(
    asset_root: &Path,
    data: &Path,
    rules: bool,
) -> Result<(LoadedUnits, Vec<CheckedBuildingSprite>), SetupError> {
    let units = load_units_file(asset_root, data_file(data, UNITS_PATH))?;
    with_match_data(units, asset_root, data, rules)
}

/// Adds the economy, combat and arsenal files of the data directory `data` to the loaded
/// `units`, and with `rules` its rules file (without one, a world with the definitions behaves
/// as before RA1), and checks the sprite of every building type under `asset_root`.
fn with_match_data(
    units: LoadedUnits,
    asset_root: &Path,
    data: &Path,
    rules: bool,
) -> Result<(LoadedUnits, Vec<CheckedBuildingSprite>), SetupError> {
    let LoadedUnits {
        definitions,
        sheets,
    } = units;
    let economy_file = data_file(data, ECONOMY_PATH);
    let definitions = definitions
        .with_economy(&economy_file)
        .and_then(|definitions| definitions.with_combat(data_file(data, COMBAT_PATH)))
        // The rules before the arsenal, which names their countries and groups (RA1).
        .and_then(|definitions| {
            if rules {
                definitions.with_rules(data_file(data, RULES_PATH))
            } else {
                Ok(definitions)
            }
        })
        .and_then(|definitions| definitions.with_arsenal(data_file(data, ARSENAL_PATH)))
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
#[cfg(test)]
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
    mut paint: crate::presentation::TeamPaint,
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
        let (Some(human), Some(enemy)) = (sprites.0.get(&kind.kind), sprites.1.get(&kind.kind))
        else {
            continue;
        };
        let image = &paint.sprite(owner.0, human, enemy);
        let view = commands
            .spawn((
                UnitView {
                    unit,
                    id: *id,
                    owner: owner.0,
                    previous: position.0,
                    current: position.0,
                    altitude: 0.0,
                    drawn: None,
                    stride: 0.0,
                },
                bevy::sprite::Anchor::CENTER,
                Sprite {
                    image: image.clone(),
                    rect: Some(crate::hd_art::facing_rect(facing.0)),
                    custom_size: Some(Vec2::splat(crate::hd_art::UNIT_SIZE)),
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
        if host
            .grid()
            .is_some_and(|grid| grid.domain_of(*id) == sim::Domain::Air)
        {
            let (offset, alpha) = air_shadow(0.0);
            commands.spawn((
                AirShadow,
                ChildOf(view),
                Sprite {
                    // The silhouette only: any copy of the sheet will do.
                    image: human.clone(),
                    rect: Some(crate::hd_art::facing_rect(facing.0)),
                    custom_size: Some(
                        Vec2::splat(crate::hd_art::UNIT_SIZE) * Vec2::new(1.0, AIR_SHADOW_SQUASH),
                    ),
                    color: Color::srgba(0.0, 0.0, 0.0, alpha),
                    ..default()
                },
                Transform::from_translation(offset.extend(0.0)),
            ));
        }
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
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn place_unit_views(
    sprites: Option<Res<UnitSprites>>,
    walks: Option<Res<WalkSprites>>,
    mut paint: crate::presentation::TeamPaint,
    map: Option<Res<MapView>>,
    time: Res<Time<Fixed>>,
    speed: Res<crate::game_speed::GameClock>,
    sink: Res<crate::sim_host::CommandSink>,
    host: Res<SimHost>,
    mut commands: Commands,
    mut registry: ResMut<UnitViews>,
    mut altitudes: ResMut<Altitudes>,
    // Disjoint from the building views' query (`crate::buildings`), which also writes
    // `Visibility`.
    mut views: Query<
        (
            Entity,
            &mut UnitView,
            &mut Sprite,
            &mut bevy::sprite::Anchor,
            &mut Transform,
            &mut Visibility,
            Option<&Children>,
        ),
        (Without<crate::buildings::BuildingView>, Without<AirShadow>),
    >,
    // Disjoint from every other view that is placed after the simulation's tick.
    mut shadows: Query<
        (&mut Sprite, &mut Transform),
        (
            With<AirShadow>,
            Without<crate::buildings::BuildingView>,
            Without<crate::construction::PlacementGhost>,
        ),
    >,
) {
    let (Some(map), Some(players)) = (map, host.players()) else {
        return;
    };
    let world = host.world();
    let fraction = time.overstep_fraction();
    let rearming = world.get_resource::<sim::SpecialState>();
    // RA4: the cell of the dock of every aircraft that has one (`sim::dock_cell`).
    let dock_cells: std::collections::BTreeMap<SimId, sim::CellPos> = rearming
        .filter(|state| !state.docks.is_empty())
        .and_then(|state| {
            let mut buildings = world.try_query::<(&SimId, &sim::Building)>()?;
            let footprints: std::collections::BTreeMap<SimId, sim::Footprint> = buildings
                .iter(world)
                .map(|(id, building)| (*id, building.footprint))
                .collect();
            Some(
                state
                    .docks
                    .iter()
                    .filter_map(|(unit, (building, dock))| {
                        Some((*unit, sim::dock_cell(*footprints.get(building)?, *dock)))
                    })
                    .collect(),
            )
        })
        .unwrap_or_default();
    for (entity, mut view, mut sprite, mut anchor, mut transform, mut visibility, children) in
        &mut views
    {
        let (Some(position), Some(facing), Some(mobile)) = (
            world.get::<Position>(view.unit),
            world.get::<Facing>(view.unit),
            world.get::<Mobile>(view.unit),
        ) else {
            // The unit no longer exists (destroyed, M4).
            registry.0.remove(&view.id);
            altitudes.0.remove(&view.id);
            commands.entity(entity).despawn();
            continue;
        };
        let disguise = world
            .get_resource::<sim::SpecialState>()
            .and_then(|state| state.disguise_for(HUMAN, view.id));
        let kind = disguise.or_else(|| world.get::<Unit>(view.unit).map(|unit| unit.kind));
        if let Some(owner) = world.get::<Owner>(view.unit) {
            view.owner = owner.0;
        }
        view.current = position.0;
        let cell = interpolate(view.previous, view.current, fraction);
        let walking = view.previous != view.current;
        // A figure with a walk cycle shows the frame its stride has reached while it moves, and
        // its standing frame otherwise. The stride follows the ground covered, not the clock.
        let gait = kind
            .zip(walks.as_ref())
            .and_then(|(kind, walks)| walks.0.get(&kind).map(|pair| (kind, pair)))
            .filter(|_| walking);
        if let Some((kind, _)) = gait {
            let step = view.drawn.map_or(0.0, |drawn| drawn.distance(cell));
            let cycle = if host
                .definitions()
                .and_then(|definitions| definitions.unit(kind))
                .is_some_and(|def| def.speed >= crate::hd_art::RUN_SPEED)
            {
                crate::hd_art::RUN_CYCLE_CELLS
            } else {
                crate::hd_art::WALK_CYCLE_CELLS
            };
            // A jump (a chrono shift, a transport unloading) is not a step.
            if step < 0.5 {
                view.stride = (view.stride + step / cycle).rem_euclid(1.0);
            }
        }
        view.drawn = Some(cell);
        let pair = match gait {
            Some((_, (human, enemy))) => Some((human, enemy)),
            None => kind
                .zip(sprites.as_ref())
                .and_then(|(kind, sprites)| sprites.0.get(&kind).zip(sprites.1.get(&kind))),
        };
        if let Some((human, enemy)) = pair {
            let image = paint.sprite(
                if disguise.is_some() {
                    HUMAN
                } else {
                    view.owner
                },
                human,
                enemy,
            );
            if sprite.image != image {
                sprite.image = image;
            }
        }
        let concealed = world
            .get_resource::<sim::SpecialState>()
            .is_some_and(|s| s.hidden.contains(&(HUMAN, view.id)));
        let shown = if unit_shown(view.owner, mobile, players) && (!concealed || disguise.is_some())
        {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != shown {
            *visibility = shown;
        }
        let mut ground = ground_point(cell, map.tile);
        let infantry = world
            .get::<Unit>(view.unit)
            .and_then(|u| {
                host.definitions()?
                    .metadata(sim::ProductionItem::Unit(u.kind))
            })
            .is_some_and(|m| m.category == sim::Category::Infantry);
        let phase = speed.clock() * 11.0 + view.id.0 as f32;
        // A figure without a walk cycle at least bobs as it goes.
        if walking && infantry && gait.is_none() && !sink.paused() {
            ground.y += phase.sin().abs() * 1.2;
        }

        let flying = host
            .grid()
            .is_some_and(|g| g.domain_of(view.id) == sim::Domain::Air);
        let (rect, size, hang) = match gait {
            Some(_) => (
                crate::hd_art::walk_rect(facing.0, crate::hd_art::walk_frame(view.stride)),
                crate::hd_art::WALK_SIZE,
                crate::hd_art::walk_anchor(),
            ),
            None => (
                crate::hd_art::facing_rect(facing.0),
                crate::hd_art::UNIT_SIZE,
                Vec2::ZERO,
            ),
        };
        if sprite.custom_size != Some(Vec2::splat(size)) {
            sprite.custom_size = Some(Vec2::splat(size));
        }
        if anchor.0 != hang {
            anchor.0 = hang;
        }
        let mut depth = unit_depth(cell);
        if flying {
            // An aircraft that stands on its dock has landed there (RA4); with the legacy
            // ruleset a strike aircraft that has flown home sits on the ground beside its base
            // while it rearms. Everything else that flies stays up.
            let landed = !walking
                && (aircraft_landed(dock_cells.get(&view.id).copied(), mobile)
                    || rearming.is_some_and(|state| state.rearming.contains_key(&view.id))
                        && host
                            .grid()
                            .is_some_and(|grid| beside_a_building(grid, mobile.cell)));
            // Game time: it stands still while the match is paused.
            view.altitude = approach_altitude(
                view.altitude,
                flight_altitude(infantry, landed),
                speed.delta(),
            );
            altitudes.0.insert(view.id, view.altitude);
            // Aircraft are drawn over everything on the ground, and drift a little in the air.
            depth += 1.0;
            let airborne = (view.altitude / HOVER_ALTITUDE).clamp(0.0, 1.0);
            ground.y += view.altitude + (phase * 0.16).sin() * 1.6 * airborne;
            let (offset, alpha) = air_shadow(view.altitude);
            for child in children.into_iter().flatten() {
                if let Ok((mut shadow, mut place)) = shadows.get_mut(*child) {
                    place.translation = offset.extend(AIR_SHADOW_Z - depth);
                    if shadow.rect != Some(rect) {
                        shadow.rect = Some(rect);
                    }
                    let colour = Color::srgba(0.0, 0.0, 0.0, alpha);
                    if shadow.color != colour {
                        shadow.color = colour;
                    }
                }
            }
        }
        transform.translation = Vec3::new(
            ground.x,
            ground.y + crate::hd_art::UNIT_GROUND_OFFSET,
            depth,
        );
        if sprite.rect != Some(rect) {
            sprite.rect = Some(rect);
        }
    }
}

/// Whether an aircraft whose dock is above `dock` (`None`: it has none) and that moves as
/// `mobile` stands on its dock (RA4): it is drawn landed there.
pub fn aircraft_landed(dock: Option<sim::CellPos>, mobile: &Mobile) -> bool {
    dock == Some(mobile.cell) && mobile.step.is_none()
}

/// Whether a building covers a cell within two cells of `cell`: where an aircraft that has
/// flown home to rearm has arrived (`sim` sends it to a cell beside its base).
fn beside_a_building(grid: &sim::Grid, cell: sim::CellPos) -> bool {
    (-2..=2).any(|dy| {
        (-2..=2).any(|dx| {
            grid.building(sim::CellPos::new(cell.x + dx, cell.y + dy))
                .is_some()
        })
    })
}

/// Logs the simulation's events. `crate::sim_host` copies every tick's events into the game's
/// message buffer, which Bevy advances once per frame, so each event is read once, also when
/// several ticks run in one frame (M5; before, events of more than two ticks per frame were
/// missed, M2 implementation debt item 2).
fn log_sim_events(mut events: MessageReader<SimEvent>) {
    for event in events.read() {
        match event {
            SimEvent::SuperweaponBuilt { .. }
            | SimEvent::SuperweaponLaunched { .. }
            | SimEvent::SuperweaponReady { .. } => {}
            SimEvent::PropDestroyed { cell, kind, .. } => {
                info!("{} at ({}, {}) destroyed", kind.name(), cell.x, cell.y);
            }
            SimEvent::StrategicActivated { player, role, .. } => {
                info!("player {} activated {role:?}", player.0)
            }
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
            SimEvent::BuildingUndeployed {
                player, building, ..
            } => info!(
                "player {}: building {} undeployed into its base unit",
                player.0, building.0
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
            SimEvent::Impact { warhead, at, air } => debug!(
                "warhead {} went off at ({}, {}){}",
                warhead.0,
                at.x,
                at.y,
                if *air { " in the air" } else { "" }
            ),
            SimEvent::DamageStateChanged { player, id, state } => {
                debug!("player {}: {} is now {state:?}", player.0, id.0);
            }
            SimEvent::UnitCrushed {
                player,
                unit,
                crusher,
                by,
                ..
            } => info!(
                "player {}: unit {} crushed by unit {} of player {}",
                player.0, unit.0, crusher.0, by.0
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
            SimEvent::RankGained { player, unit, rank } => {
                info!("player {}: unit {} rose to rank {rank}", player.0, unit.0);
            }
            SimEvent::CrateAppeared { cell } => {
                debug!("a crate appeared at ({}, {})", cell.x, cell.y);
            }
            SimEvent::CrateGone { cell } => {
                debug!("the crate at ({}, {}) is gone", cell.x, cell.y);
            }
            SimEvent::CrateTaken {
                player,
                unit,
                content,
                credits,
                ..
            } => info!(
                "player {}: unit {} took a crate: {} ({credits} credits)",
                player.0,
                unit.0,
                content.id()
            ),
            SimEvent::CrewEscaped {
                player,
                from,
                units,
                ..
            } => info!(
                "player {}: {} survivors left {}",
                player.0,
                units.len(),
                from.0
            ),
            SimEvent::UnitLaunched {
                player,
                parent,
                unit,
            } => debug!(
                "player {}: unit {} launched unit {}",
                player.0, parent.0, unit.0
            ),
            SimEvent::UnitRecovered {
                player,
                parent,
                unit,
            } => debug!(
                "player {}: unit {} took unit {} aboard",
                player.0, parent.0, unit.0
            ),
            SimEvent::MissileDetonated {
                player, unit, at, ..
            } => debug!(
                "player {}: missile {} went off at ({}, {})",
                player.0, unit.0, at.x, at.y
            ),
            SimEvent::BridgeChanged { bridge, intact, .. } => {
                debug!("bridge {} intact: {}", bridge.0, intact)
            }
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

    /// RA4: an aircraft is drawn landed exactly while it stands on its dock.
    #[test]
    fn an_aircraft_has_landed_while_it_stands_on_its_dock() {
        let at = |x, y, step| Mobile {
            speed: 26,
            cell: sim::CellPos::new(x, y),
            step,
        };
        let dock = Some(sim::CellPos::new(4, 7));
        assert!(aircraft_landed(dock, &at(4, 7, None)));
        assert!(!aircraft_landed(dock, &at(5, 7, None)), "elsewhere");
        assert!(!aircraft_landed(None, &at(4, 7, None)), "no dock");
        let leaving = sim::Step {
            to: sim::CellPos::new(5, 7),
            progress: 10,
            length: sim::ORTHOGONAL_STEP,
        };
        assert!(
            !aircraft_landed(dock, &at(4, 7, Some(leaving))),
            "taking off"
        );
        assert_eq!(flight_altitude(false, true), LANDED_ALTITUDE);
    }

    #[test]
    fn aircraft_climb_to_their_flying_height_and_land_to_rearm() {
        assert_eq!(flight_altitude(false, false), FLIGHT_ALTITUDE);
        assert_eq!(flight_altitude(true, false), HOVER_ALTITUDE);
        assert_eq!(flight_altitude(false, true), LANDED_ALTITUDE);
        // Well above a cell's height on screen, so an aircraft never reads as standing on it.
        const { assert!(HOVER_ALTITUDE >= 32.0 && FLIGHT_ALTITUDE > HOVER_ALTITUDE) };
        // A new aircraft starts on the ground and climbs at a steady rate, without overshoot.
        let mut altitude = 0.0;
        let mut seconds = 0.0;
        while altitude < FLIGHT_ALTITUDE {
            let next = approach_altitude(altitude, FLIGHT_ALTITUDE, 0.1);
            assert!(next > altitude && next <= FLIGHT_ALTITUDE);
            altitude = next;
            seconds += 0.1;
        }
        assert!((1.0..3.0).contains(&seconds), "{seconds}");
        assert_eq!(
            approach_altitude(FLIGHT_ALTITUDE, FLIGHT_ALTITUDE, 1.0),
            FLIGHT_ALTITUDE
        );
        // It descends as steadily, and a paused match (no time passes) holds it where it is.
        assert_eq!(approach_altitude(40.0, LANDED_ALTITUDE, 0.25), 28.0);
        assert_eq!(approach_altitude(40.0, LANDED_ALTITUDE, 0.0), 40.0);
        assert_eq!(
            approach_altitude(4.0, LANDED_ALTITUDE, 0.25),
            LANDED_ALTITUDE
        );
    }

    #[test]
    fn an_aircrafts_shadow_stays_on_the_ground_below_it() {
        // The view is drawn `altitude` above the ground; the shadow hangs that far below the
        // view, so on screen it does not move while the aircraft climbs.
        let (low, dark) = air_shadow(0.0);
        let (high, faint) = air_shadow(FLIGHT_ALTITUDE);
        assert_eq!(low.x, 0.0);
        assert_eq!(high.x, 0.0);
        assert_eq!(low.y - high.y, FLIGHT_ALTITUDE);
        assert_eq!(air_shadow(20.0).0.y, low.y - 20.0);
        // Higher up it is fainter, but always visible and never opaque.
        assert!(dark > faint && faint >= 0.4 && dark < 1.0, "{dark} {faint}");
        assert_eq!(air_shadow(FLIGHT_ALTITUDE * 3.0).1, faint);
        // Under every unit and building, over the terrain.
        const { assert!(AIR_SHADOW_Z < UNIT_Z && AIR_SHADOW_Z > 0.5) };
    }

    #[test]
    fn units_on_the_ground_have_no_altitude() {
        let mut altitudes = Altitudes::default();
        altitudes.0.insert(SimId(4), 64.0);
        assert_eq!(altitudes.of(SimId(4)), 64.0);
        assert_eq!(altitudes.of(SimId(5)), 0.0);
    }

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
        // unit types, in groups of at least one; for a country (RA1) they are the base unit of
        // its side and the starting unit types the skirmish file lists for the side.
        let catalogue = crate::skirmish::tests::production_catalogue();
        let rules = catalogue.definitions.rules().expect("rules");
        for faction in &catalogue.skirmish.factions {
            for group in &faction.units {
                assert!(group.count > 0);
            }
            let side = faction.side.and_then(|side| catalogue.skirmish.side(side));
            let starting: Vec<UnitKind> = faction
                .units
                .iter()
                .map(|group| group.kind)
                .chain(
                    faction
                        .side
                        .and_then(|side| rules.side(side))
                        .map(|side| side.base_unit),
                )
                .chain(side.iter().flat_map(|side| &side.vehicles).copied())
                .chain(side.iter().flat_map(|side| &side.infantry).copied())
                .collect();
            assert!(starting.len() > 1, "{}", faction.id);
            for kind in starting {
                assert_eq!(definitions.unit(kind), catalogue.definitions.unit(kind));
                assert!(definitions.unit(kind).is_some(), "{kind:?}");
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
        // The demo match has a defensive structure: a building type with a weapon (since RA1
        // the Allied pillbox).
        let armed = |kind: sim::BuildingKind| {
            definitions
                .building_combat(kind)
                .and_then(|terms| terms.weapon)
                .is_some()
        };
        let turret = definitions.building_kind("pillbox").expect("turret");
        assert!(armed(turret));
        // The computer player of the demo match builds it (M6: the starting buildings are data
        // of the skirmish file, and the computer player's buildings follow the AI file's build
        // order, D40, D42); since RA1 the plan of each side builds an armed structure of its own.
        let catalogue = crate::skirmish::tests::production_catalogue();
        assert!(
            catalogue
                .ai
                .plans
                .iter()
                .any(|plan| plan.build_order.contains(&turret))
        );
        for plan in &catalogue.ai.plans {
            assert!(
                plan.build_order.iter().any(|&kind| armed(kind)),
                "{:?}",
                plan.side
            );
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
        // M6: the starting units are those of the skirmish file's factions at the map's start
        // positions, placed by `sim::setup_match` (D40). Every one of them gets its own passable
        // cell, none on a building, and every group is complete. Since RA1 the factions are
        // countries: a player starts with the starting units of its side for the lobby's unit
        // count (`sim::starting_units`), and with the base unit of its side as the menu starts
        // the match (`menu_host`) or the yard it deploys into (`demo_host`).
        let catalogue = crate::skirmish::tests::production_catalogue();
        let selection = crate::skirmish::MenuSelection::versus(&catalogue);
        let rules = catalogue.definitions.rules().expect("rules");
        let unit_count = u32::try_from(rules.dialog.unit_count).expect("unit count");
        for mcv_start in [false, true] {
            let host = if mcv_start {
                crate::sim_host::tests::menu_host()
            } else {
                crate::sim_host::tests::demo_host()
            };
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
                let faction = &catalogue.skirmish.factions[setup.faction];
                for group in &faction.units {
                    *expected.entry((setup.player, group.kind)).or_insert(0) += group.count;
                }
                let side = faction.side.expect("a country");
                let base = rules.side(side).expect("side").base_unit;
                let starting = sim::starting_units(
                    &catalogue.skirmish,
                    &catalogue.definitions,
                    side,
                    unit_count,
                    &mut sim::SimRng::new(0),
                );
                assert!(!starting.is_empty(), "{}", faction.id);
                for kind in starting.into_iter().chain(mcv_start.then_some(base)) {
                    *expected.entry((setup.player, kind)).or_insert(0) += 1;
                }
            }
            assert!(!expected.is_empty());
            assert_eq!(placed, expected, "MCV start: {mcv_start}");
        }
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
            water: false,
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
            message.contains("the image is 704x32 px, but a sprite sheet is 512x64 px"),
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
