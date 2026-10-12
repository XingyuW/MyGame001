//! Buildings and the economy in the game (PLAN.md section 7, M3; docs/DECISIONS.md D27): the
//! building sprites, drawing the simulation's buildings, and the player's credits and power in the
//! window title. The starting buildings and credits of a match are data of the skirmish file from
//! M6 (`sim::setup_match`, D40).
//!
//! As for units (`crate::units`), the game reads simulation state and never writes it: each
//! building is drawn by a separate view entity, and the only writes into the simulation world
//! are match setup before the first tick (`crate::sim_host::install_match`) and the commands of
//! the interface, which `crate::sim_host` moves into `CommandQueue` (docs/DECISIONS.md D34).
//!
//! Every building type named by the economy file has a sprite, which must exist and pass the PNG
//! subset check at the size its footprint implies before the match starts (cross validation,
//! PLAN.md section 5; D16, D20); the renderer receives exactly the checked bytes.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use sim::{
    Building, BuildingKind, CellPos, Definitions, Footprint, Grid, Owner, PlayerId, PlayerState,
    Players, SimId,
};

use crate::map::MapView;
use crate::projection::{TileSize, to_world};
use crate::sim_host::SimHost;
use crate::tileset_image::{TilesetImageError, TilesetImageErrorKind, check_png_file};
use crate::units::HUMAN;

/// The economy file, relative to the asset directory.
pub const ECONOMY_PATH: &str = "data/economy.ron";

/// Directory of the building sprites, relative to the asset directory.
pub const SPRITE_DIR: &str = "sprites/buildings";

/// Height of a building sprite's walls in pixels (`examples/gen_building_sprites.rs`).
pub const WALL_PX: u32 = 32;

pub struct BuildingsPlugin;

impl Plugin for BuildingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BuildingViews>()
            .init_resource::<BuildingStatus>()
            .add_systems(
                Update,
                (
                    spawn_building_views,
                    // The title is written to the window that the camera's systems read (M5:
                    // ordered, so that no two game systems share state unordered).
                    show_status.before(crate::camera::pan_camera),
                ),
            )
            .add_systems(
                PostUpdate,
                update_building_views.before(bevy::transform::TransformSystems::Propagate),
            );
    }
}

/// The sprite of each building type.
#[derive(Resource, Debug, Default)]
pub struct BuildingSprites(
    pub BTreeMap<BuildingKind, Handle<Image>>,
    pub BTreeMap<BuildingKind, Handle<Image>>,
);

/// The view entity of each simulation building.
#[derive(Resource, Debug, Default)]
pub struct BuildingViews(pub BTreeMap<SimId, Entity>);

/// What match setup did with buildings; read by the smoke test.
#[derive(Resource, Debug, Default)]
pub struct BuildingStatus {
    /// Buildings spawned by match setup.
    pub spawned: usize,
}

/// The entity that draws a simulation building.
#[derive(Component, Debug)]
pub struct BuildingView {
    pub building: Entity,
    pub id: SimId,
}

/// Size in pixels of the sprite of a building of `width` × `height` cells: its footprint's
/// diamond with 64×32 pixel tiles, and the walls above it.
pub const fn sprite_size(width: u8, height: u8) -> (u32, u32) {
    let cells = width as u32 + height as u32;
    (cells * 32, cells * 16 + WALL_PX)
}

/// Path of a building sprite relative to the asset directory.
pub fn sprite_asset_path(sprite: &str) -> PathBuf {
    Path::new(SPRITE_DIR).join(format!("{sprite}.png"))
}

/// A building sprite that passed its check.
#[derive(Debug)]
pub struct CheckedBuildingSprite {
    pub kind: BuildingKind,
    pub asset_path: PathBuf,
    pub bytes: Vec<u8>,
}

/// Why a building sprite named by the economy file was rejected (cross validation).
#[derive(Debug)]
pub struct BuildingSpriteError {
    /// The economy file.
    pub file: PathBuf,
    /// The definition's `sprite` field, e.g. `buildings[1].sprite`.
    pub field: String,
    pub image: TilesetImageError,
    /// The size the building's footprint implies.
    pub expected: (u32, u32),
}

impl fmt::Display for BuildingSpriteError {
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
            TilesetImageErrorKind::SizeMismatch { width, height, .. } => write!(
                f,
                "the image is {width}x{height} px, but the building's footprint needs \
                 {}x{} px",
                self.expected.0, self.expected.1
            ),
            kind => kind.fmt(f),
        }
    }
}

impl std::error::Error for BuildingSpriteError {}

/// Checks the sprite of every building type of `definitions` under `asset_root`; `economy_file`
/// names the file the errors refer to.
pub fn check_building_sprites(
    asset_root: &Path,
    economy_file: &Path,
    definitions: &Definitions,
) -> Result<Vec<CheckedBuildingSprite>, Box<BuildingSpriteError>> {
    let mut sprites = Vec::new();
    for (index, def) in definitions.buildings().iter().enumerate() {
        let asset_path = sprite_asset_path(&def.sprite);
        let expected = sprite_size(def.width, def.height);
        let bytes = check_png_file(&asset_root.join(&asset_path), expected.0, expected.1).map_err(
            |image| {
                Box::new(BuildingSpriteError {
                    file: economy_file.to_owned(),
                    field: format!("buildings[{index}].sprite"),
                    image,
                    expected,
                })
            },
        )?;
        sprites.push(CheckedBuildingSprite {
            kind: BuildingKind(u16::try_from(index).unwrap_or(u16::MAX)),
            asset_path,
            bytes,
        });
    }
    Ok(sprites)
}

/// World position of a building's sprite: the centre of its footprint's diamond, raised by half
/// the wall height (the image's centre lies that far above the footprint's centre).
pub fn sprite_position(footprint: Footprint, tile: TileSize) -> Vec2 {
    let centre = to_world(
        footprint.origin.x as f32 + f32::from(footprint.width) / 2.0,
        footprint.origin.y as f32 + f32::from(footprint.height) / 2.0,
        tile,
    );
    centre + Vec2::new(0.0, WALL_PX as f32 / 2.0)
}

/// Draw order of a building, on the scale of `crate::units::unit_depth`: by the centre of its
/// footprint.
pub fn building_depth(footprint: Footprint) -> f32 {
    let u = footprint.origin.x as f32 + f32::from(footprint.width) / 2.0;
    let v = footprint.origin.y as f32 + f32::from(footprint.height) / 2.0;
    crate::units::unit_depth(Vec2::new(u, v))
}

/// Creates a view for every simulation building that has none. Buildings do not move, so the
/// view is placed once.
pub(crate) fn spawn_building_views(
    mut commands: Commands,
    mut views: ResMut<BuildingViews>,
    map: Option<Res<MapView>>,
    sprites: Option<Res<BuildingSprites>>,
    host: Res<SimHost>,
    mut paint: crate::presentation::TeamPaint,
) {
    let (Some(map), Some(sprites)) = (map, sprites) else {
        return;
    };
    let Some(mut buildings) = host
        .world()
        .try_query::<(Entity, &SimId, &Owner, &Building)>()
    else {
        return;
    };
    for (building, id, owner, component) in buildings.iter(host.world()) {
        if views.0.contains_key(id) {
            continue;
        }
        let (Some(human), Some(enemy)) = (
            sprites.0.get(&component.kind),
            sprites.1.get(&component.kind),
        ) else {
            continue;
        };
        let image = &paint.sprite(owner.0, human, enemy);
        let position = crate::hd_art::building_position(component.footprint, map.tile);
        let view = commands
            .spawn((
                BuildingView { building, id: *id },
                Sprite {
                    image: image.clone(),
                    custom_size: Some(crate::hd_art::building_size(
                        component.footprint.width,
                        component.footprint.height,
                    )),
                    color: Color::WHITE,
                    ..default()
                },
                Transform::from_translation(position.extend(building_depth(component.footprint))),
                // Another player's building is shown only once `update_building_views` finds it
                // visible.
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

/// Whether the human player is shown a building of `owner` with this footprint: its own
/// buildings always, other players' buildings only while the human player sees some cell of
/// them (M4; docs/DECISIONS.md D31).
pub fn building_shown(owner: PlayerId, footprint: Footprint, players: &Players) -> bool {
    owner == HUMAN
        || players
            .get(HUMAN)
            .is_some_and(|state| state.vision().sees_footprint(footprint))
}

/// Removes the views of buildings that no longer exist (sold or destroyed), and hides the views
/// of other players' buildings while the human player does not see them.
fn update_building_views(
    sprites: Option<Res<BuildingSprites>>,
    mut paint: crate::presentation::TeamPaint,
    mut commands: Commands,
    mut registry: ResMut<BuildingViews>,
    host: Res<SimHost>,
    // Disjoint from the unit views' query (`crate::units`), which also writes `Visibility`.
    mut views: Query<
        (Entity, &BuildingView, &mut Visibility, &mut Sprite),
        Without<crate::units::UnitView>,
    >,
) {
    let Some(players) = host.players() else {
        return;
    };
    let world = host.world();
    for (entity, view, mut visibility, mut sprite) in &mut views {
        let (Some(owner), Some(building)) = (
            world.get::<Owner>(view.building),
            world.get::<Building>(view.building),
        ) else {
            registry.0.remove(&view.id);
            commands.entity(entity).despawn();
            continue;
        };
        if let Some(sprites) = sprites.as_ref()
            && let (Some(human), Some(enemy)) =
                (sprites.0.get(&building.kind), sprites.1.get(&building.kind))
        {
            let image = paint.sprite(owner.0, human, enemy);
            if sprite.image != image {
                sprite.image = image;
            }
        }
        let shown = if building_shown(owner.0, building.footprint, players) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != shown {
            *visibility = shown;
        }
    }
}

/// The window title: the game's name, and the player's credits and power.
pub fn status_title(state: Option<&PlayerState>) -> String {
    match state {
        None => "MyGame001".into(),
        Some(state) => format!(
            "MyGame001 - credits {} - power {}/{}{}",
            state.credits(),
            state.power_consumed(),
            state.power_produced(),
            if state.low_power() { " (low)" } else { "" }
        ),
    }
}

/// Shows the player's credits and power in the window title (the sidebar arrives in M5).
fn show_status(host: Res<SimHost>, mut window: Single<&mut Window, With<PrimaryWindow>>) {
    let title = status_title(host.players().and_then(|players| players.get(HUMAN)));
    if window.title != title {
        window.title = title;
    }
}

/// The building of `player` covering `cell`, if any.
pub fn own_building_at(
    grid: &Grid,
    owners: &BTreeMap<SimId, PlayerId>,
    player: PlayerId,
    cell: CellPos,
) -> Option<SimId> {
    grid.building(cell)
        .filter(|id| owners.get(id) == Some(&player))
}

/// The sprite generator, to tie the sprite layout to it in the tests.
#[cfg(test)]
#[path = "../examples/gen_building_sprites.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod gen_building_sprites;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projection::cell_centre;

    fn assets_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    fn production_definitions() -> Definitions {
        sim::load_definitions(
            assets_root().join(crate::units::UNITS_PATH),
            assets_root().join(ECONOMY_PATH),
        )
        .unwrap_or_else(|error| panic!("{error}"))
    }

    #[test]
    fn the_sprite_layout_matches_the_generator() {
        assert_eq!(WALL_PX, gen_building_sprites::WALL_PX);
        for width in 1..=sim::MAX_FOOTPRINT_SIDE {
            for height in 1..=sim::MAX_FOOTPRINT_SIDE {
                assert_eq!(
                    sprite_size(width, height),
                    gen_building_sprites::image_size(width, height)
                );
            }
        }
        // Every production building has a generator entry with its footprint, so its
        // placeholder is reproducible.
        for def in production_definitions().buildings() {
            let entry = gen_building_sprites::SPRITES
                .iter()
                .find(|sprite| sprite.name == def.sprite)
                .unwrap_or_else(|| panic!("{}", def.sprite));
            assert_eq!((entry.width, entry.height), (def.width, def.height));
        }
    }

    #[test]
    fn the_production_buildings_load_and_every_sprite_passes_its_check() {
        let definitions = production_definitions();
        let file = assets_root().join(ECONOMY_PATH);
        let sprites = check_building_sprites(&assets_root(), &file, &definitions)
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(sprites.len(), definitions.buildings().len());
        for (sprite, def) in sprites.iter().zip(definitions.buildings()) {
            assert_eq!(definitions.building(sprite.kind), Some(def));
            assert_eq!(sprite.asset_path, sprite_asset_path(&def.sprite));
            let bytes = std::fs::read(assets_root().join(&sprite.asset_path)).expect("sprite");
            assert_eq!(sprite.bytes, bytes);
        }
        // What every faction of the skirmish file starts with (M6: data, D40) names defined
        // building types: its starting buildings, or for a country (RA1) the Construction Yard
        // that the base unit of its side deploys into.
        let catalogue = crate::skirmish::tests::production_catalogue();
        let rules = catalogue.definitions.rules().expect("rules");
        for faction in &catalogue.skirmish.factions {
            let starting: Vec<sim::BuildingKind> = faction
                .buildings
                .iter()
                .map(|building| building.kind)
                .chain(
                    faction
                        .side
                        .and_then(|side| rules.side(side))
                        .map(|side| side.construction_yard),
                )
                .collect();
            assert!(!starting.is_empty(), "{}", faction.id);
            for kind in starting {
                assert_eq!(
                    definitions.building(kind),
                    catalogue.definitions.building(kind)
                );
                assert!(definitions.building(kind).is_some(), "{kind:?}");
                assert!(faction.starts_with(&catalogue.definitions, kind));
            }
        }
    }

    #[test]
    fn a_missing_or_wrongly_sized_building_sprite_is_reported_with_the_data_file_and_field() {
        let dir = std::env::temp_dir().join(format!(
            "mygame001_game_buildings_tests_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(SPRITE_DIR)).expect("sprite directory");
        let definitions = production_definitions();
        let file = assets_root().join(ECONOMY_PATH);
        let message = check_building_sprites(&dir, &file, &definitions)
            .expect_err("no sprite")
            .to_string();
        assert!(message.contains("buildings[0].sprite"), "{message}");
        assert!(message.contains("construction_yard.png"), "{message}");
        assert!(message.contains("cannot be read"), "{message}");
        // The 3x3 yard needs a 192x128 image; the 2x2 power plant's sprite is 128x96.
        std::fs::copy(
            assets_root().join(SPRITE_DIR).join("power_plant.png"),
            dir.join(SPRITE_DIR).join("construction_yard.png"),
        )
        .expect("copy");
        let message = check_building_sprites(&dir, &file, &definitions)
            .expect_err("wrong size")
            .to_string();
        assert!(
            message
                .contains("the image is 128x96 px, but the building's footprint needs 192x128 px"),
            "{message}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_sprite_stands_on_its_footprint() {
        let tile = TileSize::new(64, 32);
        // A 1x1 building: the footprint's centre is the cell's centre, and the image's centre
        // lies half the wall height above it.
        let one = Footprint {
            origin: CellPos::new(3, 5),
            width: 1,
            height: 1,
        };
        assert_eq!(
            sprite_position(one, tile),
            cell_centre(CellPos::new(3, 5), tile) + Vec2::new(0.0, 16.0)
        );
        // A 3x2 building: the image's bottom edge is the footprint's bottom corner, and its left
        // and right edges are the footprint's left and right corners.
        let footprint = Footprint {
            origin: CellPos::new(10, 4),
            width: 3,
            height: 2,
        };
        let (width, height) = sprite_size(3, 2);
        let centre = sprite_position(footprint, tile);
        let bottom = to_world(13.0, 6.0, tile);
        let left = to_world(10.0, 6.0, tile);
        let right = to_world(13.0, 4.0, tile);
        assert_eq!(centre.y - height as f32 / 2.0, bottom.y);
        assert_eq!(centre.x - width as f32 / 2.0, left.x);
        assert_eq!(centre.x + width as f32 / 2.0, right.x);
    }

    #[test]
    fn the_title_shows_credits_and_power() {
        assert_eq!(status_title(None), "MyGame001");
        let mut players = Players::default();
        players.add(HUMAN, 1_234).expect("player");
        assert_eq!(
            status_title(players.get(HUMAN)),
            "MyGame001 - credits 1234 - power 0/0"
        );
    }

    #[test]
    fn the_demo_map_holds_the_starting_buildings_and_units() {
        // Match setup on the demo map (M6: `sim::setup_match` with the skirmish file, D40): every
        // starting building of both factions fits (none since RA1: a country starts with its
        // base unit), every starting unit still finds a free cell around its anchor and none
        // stands on a building, and both players start with the skirmish file's credits.
        let catalogue = crate::skirmish::tests::production_catalogue();
        let selection = crate::skirmish::MenuSelection {
            mcv_start: false,
            ..crate::skirmish::MenuSelection::versus(&catalogue)
        };
        let host = crate::sim_host::tests::demo_host();
        let world = host.world();
        let grid = host.grid().expect("grid");
        let (expected_units, expected_buildings) =
            crate::skirmish::selection_totals(&catalogue, selection).expect("fixed countries");
        let mut buildings = world.try_query::<&Building>().expect("registered");
        assert_eq!(buildings.iter(world).count(), expected_buildings);
        let mut units = world.try_query::<&sim::Mobile>().expect("registered");
        assert_eq!(units.iter(world).count(), expected_units);
        for mobile in units.iter(world) {
            assert_eq!(grid.building(mobile.cell), None);
        }
        for setup in selection.players() {
            assert_eq!(
                host.players()
                    .and_then(|players| players.get(setup.player))
                    .map(PlayerState::credits),
                Some(catalogue.skirmish.starting_credits)
            );
        }
    }
}
