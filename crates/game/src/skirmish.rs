//! Game flow (PLAN.md section 7, M6): the catalogue of maps, factions, and difficulties loaded at
//! startup, the main menu with map, faction, and difficulty selection, the start of a match against
//! the computer player, the end screen with the winner, and restart (docs/DECISIONS.md D44).
//!
//! When the game starts, [`load_catalogue`] loads and checks everything a match can use: the unit,
//! economy, and combat files with every sprite (`crate::units::load_match_data`), the skirmish file
//! (factions, maps, start positions, starting credits) and the AI file (`sim::load_skirmish`,
//! `sim::load_ai`), and every map of the skirmish file with its tileset image
//! (`crate::map::check_map`). The renderer receives only the checked bytes, which are registered
//! once (D16); nothing is read again when a match starts or restarts. Any error ends the game with
//! exit code 1, as before M6.
//!
//! The menu chooses a map, the human player's faction, the computer player's faction, and the
//! difficulty ([`menu_effect`], pure). Starting a match ([`begin_match`]) ends the current one if
//! any ([`end_match`]: the map, the unit and building views, and every resource of the match are
//! removed or reset), sets up a new simulation with `sim::setup_match` through
//! `crate::sim_host::install_match` (human player 0 at the map's first start position, the
//! computer player 1 at the second), spawns the map, sizes the minimap, and centres the camera on
//! the human player's start. The seed of the simulation is taken from the clock when the match
//! starts and logged (M0 debt item 3). When the simulation reports the match's outcome, ticks stop
//! and the end screen shows victory, defeat, or a draw, with Restart (the same choices, a new seed)
//! and Main menu ([`end_effect`], pure).

use std::path::{Path, PathBuf};

use bevy::asset::io::file::FileAssetReader;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use sim::{
    AiDefinitions, Controller, Definitions, MatchOutcome, PlayerSetup, SkirmishDefinitions,
    map_asset_path,
};

use crate::buildings::{BuildingSprites, BuildingStatus, BuildingView, BuildingViews};
use crate::camera::CameraBounds;
use crate::checked_assets::CheckedAssets;
use crate::construction::InterfaceMode;
use crate::fog::RecentShots;
use crate::hotkeys::ControlGroups;
use crate::map::{CheckedMap, check_map};
use crate::projection::{cell_centre, map_bounds};
use crate::selection::{DragBox, Selection};
use crate::sim_host::{MatchPlan, SimHost};
use crate::units::{HUMAN, OPPONENT, UnitSprites, UnitStatus, UnitView, UnitViews};

/// The skirmish file, relative to the asset directory.
pub const SKIRMISH_PATH: &str = "data/skirmish.ron";

/// The AI file, relative to the asset directory.
pub const AI_PATH: &str = "data/ai.ron";

pub struct SkirmishPlugin;

impl Plugin for SkirmishPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Screen>()
            .init_resource::<MenuSelection>()
            .init_resource::<LastMatch>()
            .add_systems(Startup, load_catalogue)
            .add_systems(Startup, spawn_screens.after(crate::sidebar::spawn_sidebar))
            .add_systems(
                Update,
                (screen_input, update_screens)
                    .chain()
                    // Both write texts and nodes of the interface.
                    .after(crate::sidebar::update_sidebar)
                    .after(crate::minimap::place_camera_frame),
            );
    }
}

// --- The catalogue --------------------------------------------------------------------------

/// A map of the catalogue: its id, its file, and its checked grid and size.
#[derive(Clone, Debug)]
pub struct CatalogueMap {
    pub id: String,
    /// The map file, relative to the asset directory.
    pub asset_path: PathBuf,
    pub map: CheckedMap,
}

/// Everything a match can use, loaded and checked when the game starts.
#[derive(Resource, Clone, Debug)]
pub struct Catalogue {
    pub definitions: Definitions,
    pub skirmish: SkirmishDefinitions,
    pub ai: AiDefinitions,
    /// The maps of the skirmish file, in its order.
    pub maps: Vec<CatalogueMap>,
}

/// Loads the catalogue from the asset directory `root`: the definitions with every sprite, the
/// skirmish and AI files, and every map of the skirmish file with its tileset image. The checked
/// bytes of the sprites and maps are registered for the renderer; the sprite handles are returned
/// for the views.
pub fn load(
    root: &Path,
    checked: &CheckedAssets,
) -> Result<
    (
        Catalogue,
        Vec<crate::units::CheckedSheet>,
        Vec<crate::buildings::CheckedBuildingSprite>,
    ),
    String,
> {
    let (loaded, buildings) =
        crate::units::load_match_data(root).map_err(|error| error.to_string())?;
    let definitions = loaded.definitions;
    let skirmish = sim::load_skirmish(root.join(SKIRMISH_PATH), &definitions)
        .map_err(|error| error.to_string())?;
    let ai = sim::load_ai(root.join(AI_PATH), &definitions).map_err(|error| error.to_string())?;
    let mut maps = Vec::with_capacity(skirmish.maps.len());
    for def in &skirmish.maps {
        let asset_path = map_asset_path(&def.id);
        let map = check_map(root, &asset_path, checked).map_err(|error| {
            format!(
                "data file {}: map {:?}: {error}",
                root.join(SKIRMISH_PATH).display(),
                def.id
            )
        })?;
        maps.push(CatalogueMap {
            id: def.id.clone(),
            asset_path,
            map,
        });
    }
    Ok((
        Catalogue {
            definitions,
            skirmish,
            ai,
            maps,
        },
        loaded.sheets,
        buildings,
    ))
}

/// Startup: loads the catalogue ([`load`]) and registers the sprites. An error is logged and ends
/// the game with exit code 1.
pub(crate) fn load_catalogue(world: &mut World) {
    let root = FileAssetReader::new(AssetPlugin::default().file_path)
        .root_path()
        .clone();
    let checked = world.resource::<CheckedAssets>().clone();
    let (catalogue, sheets, buildings) = match load(&root, &checked) {
        Ok(loaded) => loaded,
        Err(error) => {
            error!("{error}");
            world.write_message(AppExit::error());
            return;
        }
    };
    let server = world.resource::<AssetServer>().clone();
    let mut sprites = BuildingSprites::default();
    for sprite in buildings {
        checked.insert(sprite.asset_path.clone(), sprite.bytes);
        sprites
            .0
            .insert(sprite.kind, server.load(sprite.asset_path));
    }
    world.insert_resource(sprites);
    let mut sprites = UnitSprites::default();
    for sheet in sheets {
        checked.insert(sheet.asset_path.clone(), sheet.bytes);
        sprites.0.insert(sheet.kind, server.load(sheet.asset_path));
    }
    world.insert_resource(sprites);
    let selection = MenuSelection::initial(&catalogue);
    info!(
        "catalogue: {} maps, {} factions, {} difficulties",
        catalogue.maps.len(),
        catalogue.skirmish.factions.len(),
        catalogue.ai.difficulties.len()
    );
    world.insert_resource(selection);
    world.insert_resource(catalogue);
}

// --- Choices and their pure functions -------------------------------------------------------

/// What the game shows: the menu, or a match (running, or ended under the end screen).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Screen {
    #[default]
    Menu,
    Match,
}

/// A row of the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Row {
    Map,
    Faction,
    Opponent,
    Difficulty,
}

impl Row {
    pub const ALL: [Row; 4] = [Row::Map, Row::Faction, Row::Opponent, Row::Difficulty];

    pub fn label(self) -> &'static str {
        match self {
            Row::Map => "Map",
            Row::Faction => "Your faction",
            Row::Opponent => "Opponent",
            Row::Difficulty => "Difficulty",
        }
    }
}

/// The choices of a match: indices into the catalogue's maps, factions, and difficulties.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuSelection {
    pub map: usize,
    pub faction: usize,
    pub opponent: usize,
    pub difficulty: usize,
}

/// How many entries each row has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuCounts {
    pub maps: usize,
    pub factions: usize,
    pub difficulties: usize,
}

impl MenuCounts {
    pub fn of(catalogue: &Catalogue) -> Self {
        Self {
            maps: catalogue.maps.len(),
            factions: catalogue.skirmish.factions.len(),
            difficulties: catalogue.ai.difficulties.len(),
        }
    }

    fn of_row(self, row: Row) -> usize {
        match row {
            Row::Map => self.maps,
            Row::Faction | Row::Opponent => self.factions,
            Row::Difficulty => self.difficulties,
        }
    }
}

impl MenuSelection {
    /// The first map and faction for both players, and the middle difficulty (the AI file lists
    /// them from the easiest).
    pub fn initial(catalogue: &Catalogue) -> Self {
        Self {
            difficulty: catalogue.ai.difficulties.len() / 2,
            ..Self::default()
        }
    }

    fn get(self, row: Row) -> usize {
        match row {
            Row::Map => self.map,
            Row::Faction => self.faction,
            Row::Opponent => self.opponent,
            Row::Difficulty => self.difficulty,
        }
    }

    fn set(&mut self, row: Row, value: usize) {
        match row {
            Row::Map => self.map = value,
            Row::Faction => self.faction = value,
            Row::Opponent => self.opponent = value,
            Row::Difficulty => self.difficulty = value,
        }
    }

    /// Whether every choice names an entry of the catalogue.
    pub fn valid(self, counts: MenuCounts) -> bool {
        Row::ALL
            .into_iter()
            .all(|row| self.get(row) < counts.of_row(row))
    }

    /// The players of the match: the human player at the map's first start position, the
    /// computer player at the second.
    pub fn players(self) -> [PlayerSetup; 2] {
        [
            PlayerSetup {
                player: HUMAN,
                faction: self.faction,
                start: 0,
                controller: Controller::Human,
            },
            PlayerSetup {
                player: OPPONENT,
                faction: self.opponent,
                start: 1,
                controller: Controller::Ai {
                    difficulty: self.difficulty,
                },
            },
        ]
    }
}

/// A button of the menu.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuButton {
    /// The previous entry of the row (wrapping around).
    Previous(Row),
    /// The next entry of the row (wrapping around).
    Next(Row),
    Start,
    Quit,
}

/// What a menu button does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuEffect {
    Select(MenuSelection),
    Start(MenuSelection),
    Quit,
    Nothing,
}

/// The effect of `button` with the current `selection` and a catalogue of `counts` entries. A
/// selection outside the catalogue (which the buttons cannot make) starts nothing.
pub fn menu_effect(button: MenuButton, selection: MenuSelection, counts: MenuCounts) -> MenuEffect {
    let step = |row: Row, forward: bool| {
        let count = counts.of_row(row);
        if count == 0 {
            return MenuEffect::Nothing;
        }
        let current = selection.get(row).min(count - 1);
        let mut next = selection;
        next.set(
            row,
            if forward {
                (current + 1) % count
            } else {
                (current + count - 1) % count
            },
        );
        MenuEffect::Select(next)
    };
    match button {
        MenuButton::Previous(row) => step(row, false),
        MenuButton::Next(row) => step(row, true),
        MenuButton::Start if selection.valid(counts) => MenuEffect::Start(selection),
        MenuButton::Start => MenuEffect::Nothing,
        MenuButton::Quit => MenuEffect::Quit,
    }
}

/// The text of a menu row: the id of the chosen entry, with underscores as spaces.
pub fn row_text(row: Row, selection: MenuSelection, catalogue: &Catalogue) -> String {
    let id = match row {
        Row::Map => catalogue.maps.get(selection.map).map(|map| map.id.as_str()),
        Row::Faction => catalogue
            .skirmish
            .factions
            .get(selection.faction)
            .map(|faction| faction.id.as_str()),
        Row::Opponent => catalogue
            .skirmish
            .factions
            .get(selection.opponent)
            .map(|faction| faction.id.as_str()),
        Row::Difficulty => catalogue
            .ai
            .difficulties
            .get(selection.difficulty)
            .map(|difficulty| difficulty.id.as_str()),
    };
    id.map_or_else(|| "-".to_string(), |id| id.replace('_', " "))
}

/// The choices of the last match started, for Restart.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LastMatch(pub Option<MenuSelection>);

/// A button of the end screen.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndButton {
    Restart,
    Menu,
}

/// What an end screen button does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndEffect {
    /// A new match with the same choices.
    Restart(MenuSelection),
    /// Back to the menu.
    Menu,
}

/// The effect of `button` after a match started with `last`; Restart without a last match goes
/// to the menu.
pub fn end_effect(button: EndButton, last: Option<MenuSelection>) -> EndEffect {
    match (button, last) {
        (EndButton::Restart, Some(selection)) => EndEffect::Restart(selection),
        _ => EndEffect::Menu,
    }
}

/// The headline of the end screen for the human player, or `None` while the match goes on.
pub fn end_text(outcome: Option<MatchOutcome>) -> Option<&'static str> {
    match outcome? {
        MatchOutcome::Won { winner, .. } if winner == HUMAN => Some("VICTORY"),
        MatchOutcome::Won { .. } => Some("DEFEAT"),
        MatchOutcome::Draw { .. } => Some("DRAW"),
    }
}

/// A seed for a new match from the clock (the simulation itself never reads the clock, I4).
fn clock_seed() -> u64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        // The low 64 bits of the nanoseconds: enough to differ between matches.
        Ok(elapsed) => elapsed.as_nanos() as u64,
        Err(error) => {
            warn!("the clock lies before 1970 ({error}); the match seed is 0");
            0
        }
    }
}

// --- Starting and ending matches ------------------------------------------------------------

/// Starts a match with `selection` and the seed `seed`: ends the current match if any
/// ([`end_match`]), sets up the simulation, spawns the map, sizes the minimap, and centres the
/// camera on the human player's start. A setup error is logged and ends the game with exit code
/// 1, as before M6; nothing of a new match is then shown.
pub(crate) fn begin_match(world: &mut World, selection: MenuSelection, seed: u64) {
    end_match(world);
    let Some(catalogue) = world.get_resource::<Catalogue>().cloned() else {
        error!("no catalogue is loaded; no match can start");
        world.write_message(AppExit::error());
        return;
    };
    let Some(entry) = catalogue.maps.get(selection.map) else {
        error!("map {} is not in the catalogue", selection.map);
        world.write_message(AppExit::error());
        return;
    };
    let plan = MatchPlan {
        grid: entry.map.grid.clone(),
        definitions: catalogue.definitions.clone(),
        skirmish: catalogue.skirmish.clone(),
        ai: catalogue.ai.clone(),
        map: selection.map,
        players: selection.players().to_vec(),
        seed,
    };
    let start = match crate::sim_host::install_match(world, &plan) {
        Ok(start) => start,
        Err(error) => {
            error!("map {}: {error}", entry.id);
            world.write_message(AppExit::error());
            return;
        }
    };
    info!(
        "match on map {} (seed {seed}): {} buildings and {} units spawned",
        entry.id, start.buildings, start.units
    );
    world.resource_mut::<BuildingStatus>().spawned = start.buildings;
    world.resource_mut::<UnitStatus>().spawned = start.units;
    let view = entry.map.view;
    crate::map::spawn_map(world, &entry.asset_path, &entry.map);
    world.insert_resource(CameraBounds(Some(map_bounds(
        view.width,
        view.height,
        view.tile,
    ))));
    crate::minimap::size_minimap(world, view.width, view.height);
    if let Some(start) = catalogue
        .skirmish
        .maps
        .get(selection.map)
        .map(|map| map.starts[0])
    {
        let centre = cell_centre(start, view.tile);
        let cameras: Vec<Entity> = world
            .query_filtered::<Entity, With<Camera2d>>()
            .iter(world)
            .collect();
        for camera in cameras {
            if let Some(mut transform) = world.get_mut::<Transform>(camera) {
                transform.translation = centre.extend(transform.translation.z);
            }
        }
    }
    world.insert_resource(LastMatch(Some(selection)));
    *world.resource_mut::<Screen>() = Screen::Match;
}

/// Ends the current match, if any, and shows the menu: the simulation is replaced by one without
/// a match, and the map, the unit and building views, and the interface state of the match are
/// removed or reset, so that nothing of it reaches the next match.
pub(crate) fn end_match(world: &mut World) {
    crate::sim_host::uninstall_match(world);
    crate::map::despawn_map(world);
    let views: Vec<Entity> = world
        .query_filtered::<Entity, Or<(With<UnitView>, With<BuildingView>)>>()
        .iter(world)
        .collect();
    for view in views {
        world.despawn(view);
    }
    world.insert_resource(UnitViews::default());
    world.insert_resource(BuildingViews::default());
    world.insert_resource(UnitStatus::default());
    world.insert_resource(BuildingStatus::default());
    world.insert_resource(Selection::default());
    world.insert_resource(DragBox::default());
    world.insert_resource(ControlGroups::default());
    world.insert_resource(InterfaceMode::default());
    world.insert_resource(RecentShots::default());
    world.insert_resource(CameraBounds::default());
    crate::minimap::size_minimap(world, 1, 1);
    *world.resource_mut::<Screen>() = Screen::Menu;
}

// --- The screens ----------------------------------------------------------------------------

/// The menu's full-window panel.
#[derive(Component, Debug)]
pub struct MenuRoot;

/// The end screen's panel.
#[derive(Component, Debug)]
pub struct EndRoot;

/// A text of the menu or the end screen that changes.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenText {
    /// The chosen entry of a menu row.
    Row(Row),
    /// The end screen's headline.
    Outcome,
}

fn screen_text(value: impl Into<String>, size: f32) -> (Text, TextFont, TextColor) {
    (
        Text::new(value),
        TextFont::from_font_size(size),
        TextColor(Color::srgb(0.92, 0.94, 0.97)),
    )
}

fn screen_button(width: f32) -> impl Bundle {
    (
        Button,
        Node {
            width: Val::Px(width),
            height: Val::Px(32.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgb(0.24, 0.26, 0.3)),
    )
}

/// Spawns the menu (shown at first) and the end screen (hidden), above the sidebar.
pub(crate) fn spawn_screens(mut commands: Commands) {
    commands
        .spawn((
            MenuRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(12.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.05, 0.06, 0.08)),
            GlobalZIndex(10),
            FocusPolicy::Block,
        ))
        .with_children(|menu| {
            menu.spawn(screen_text("MyGame001", 44.0));
            menu.spawn(screen_text("Skirmish against the computer", 18.0));
            for row in Row::ALL {
                menu.spawn(Node {
                    column_gap: Val::Px(10.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|line| {
                    line.spawn((
                        Node {
                            width: Val::Px(140.0),
                            ..default()
                        },
                        children![screen_text(row.label(), 18.0)],
                    ));
                    line.spawn((MenuButton::Previous(row), screen_button(36.0)))
                        .with_child(screen_text("<", 18.0));
                    line.spawn((
                        Node {
                            width: Val::Px(200.0),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        children![(ScreenText::Row(row), screen_text("", 18.0))],
                    ));
                    line.spawn((MenuButton::Next(row), screen_button(36.0)))
                        .with_child(screen_text(">", 18.0));
                });
            }
            menu.spawn((MenuButton::Start, screen_button(200.0)))
                .with_child(screen_text("Start", 20.0));
            menu.spawn((MenuButton::Quit, screen_button(200.0)))
                .with_child(screen_text("Quit", 18.0));
        });
    commands
        .spawn((
            EndRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(crate::sidebar::SIDEBAR_WIDTH),
                top: Val::Percent(30.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(12.0),
                display: Display::None,
                ..default()
            },
            GlobalZIndex(9),
        ))
        .with_children(|end| {
            end.spawn((ScreenText::Outcome, screen_text("", 56.0)));
            end.spawn((EndButton::Restart, screen_button(200.0)))
                .with_child(screen_text("Restart", 20.0));
            end.spawn((EndButton::Menu, screen_button(200.0)))
                .with_child(screen_text("Main menu", 20.0));
        });
}

/// Clicks on the menu's and the end screen's buttons.
#[allow(clippy::too_many_arguments)]
fn screen_input(
    mut commands: Commands,
    screen: Res<Screen>,
    catalogue: Option<Res<Catalogue>>,
    mut selection: ResMut<MenuSelection>,
    last: Res<LastMatch>,
    host: Res<SimHost>,
    menu: Query<(&Interaction, &MenuButton), Changed<Interaction>>,
    end: Query<(&Interaction, &EndButton), Changed<Interaction>>,
) {
    match *screen {
        Screen::Menu => {
            let Some(catalogue) = catalogue else {
                return;
            };
            let counts = MenuCounts::of(&catalogue);
            for (_, &button) in menu
                .iter()
                .filter(|(interaction, _)| **interaction == Interaction::Pressed)
            {
                match menu_effect(button, *selection, counts) {
                    MenuEffect::Select(next) => *selection = next,
                    MenuEffect::Start(choice) => {
                        commands.queue(move |world: &mut World| {
                            begin_match(world, choice, clock_seed());
                        });
                    }
                    MenuEffect::Quit => {
                        commands.queue(|world: &mut World| {
                            world.write_message(AppExit::Success);
                        });
                    }
                    MenuEffect::Nothing => {}
                }
            }
        }
        Screen::Match if host.outcome().is_some() => {
            for (_, &button) in end
                .iter()
                .filter(|(interaction, _)| **interaction == Interaction::Pressed)
            {
                match end_effect(button, last.0) {
                    EndEffect::Restart(choice) => commands.queue(move |world: &mut World| {
                        begin_match(world, choice, clock_seed());
                    }),
                    EndEffect::Menu => commands.queue(end_match),
                }
            }
        }
        Screen::Match => {}
    }
}

/// Shows the menu or the end screen, with their texts.
#[allow(clippy::type_complexity)]
fn update_screens(
    screen: Res<Screen>,
    selection: Res<MenuSelection>,
    catalogue: Option<Res<Catalogue>>,
    host: Res<SimHost>,
    mut roots: Query<(&mut Node, Has<MenuRoot>), Or<(With<MenuRoot>, With<EndRoot>)>>,
    mut texts: Query<(&mut Text, &ScreenText)>,
) {
    let outcome = end_text(host.outcome());
    for (mut node, menu) in &mut roots {
        let shown = if menu {
            *screen == Screen::Menu
        } else {
            *screen == Screen::Match && outcome.is_some()
        };
        let display = if shown { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    }
    for (mut text, kind) in &mut texts {
        let value = match kind {
            ScreenText::Row(row) => catalogue.as_deref().map_or_else(String::new, |catalogue| {
                row_text(*row, *selection, catalogue)
            }),
            ScreenText::Outcome => outcome.unwrap_or_default().to_string(),
        };
        if text.0 != value {
            text.0 = value;
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use sim::PlayerId;

    use super::*;

    fn assets() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    pub(crate) fn production_catalogue() -> Catalogue {
        load(&assets(), &CheckedAssets::default())
            .unwrap_or_else(|error| panic!("{error}"))
            .0
    }

    const COUNTS: MenuCounts = MenuCounts {
        maps: 2,
        factions: 3,
        difficulties: 3,
    };

    #[test]
    fn the_production_catalogue_loads_every_map_faction_and_difficulty() {
        let catalogue = production_catalogue();
        assert!(!catalogue.maps.is_empty());
        assert_eq!(catalogue.maps.len(), catalogue.skirmish.maps.len());
        for (entry, def) in catalogue.maps.iter().zip(&catalogue.skirmish.maps) {
            assert_eq!(entry.id, def.id);
            assert_eq!(entry.asset_path, map_asset_path(&def.id));
            // Both start positions lie on the map.
            for start in def.starts {
                assert!(entry.map.grid.contains(start), "{start:?}");
            }
        }
        assert!(!catalogue.skirmish.factions.is_empty());
        assert!(!catalogue.ai.difficulties.is_empty());
        // The menu starts with the middle difficulty.
        let selection = MenuSelection::initial(&catalogue);
        assert_eq!(selection.difficulty, catalogue.ai.difficulties.len() / 2);
        assert!(selection.valid(MenuCounts::of(&catalogue)));
    }

    #[test]
    fn every_shipped_match_can_be_set_up_for_every_faction_pairing() {
        // Cross validation of the shipped data: every faction fits at both start positions of
        // every map, against every other faction (both orders) and with every difficulty.
        let catalogue = production_catalogue();
        let counts = MenuCounts::of(&catalogue);
        for map in 0..counts.maps {
            for faction in 0..counts.factions {
                for opponent in 0..counts.factions {
                    for difficulty in 0..counts.difficulties {
                        let selection = MenuSelection {
                            map,
                            faction,
                            opponent,
                            difficulty,
                        };
                        let mut world = World::new();
                        world.init_resource::<crate::sim_host::CommandSink>();
                        let plan = MatchPlan {
                            grid: catalogue.maps[map].map.grid.clone(),
                            definitions: catalogue.definitions.clone(),
                            skirmish: catalogue.skirmish.clone(),
                            ai: catalogue.ai.clone(),
                            map,
                            players: selection.players().to_vec(),
                            seed: 0,
                        };
                        let start = crate::sim_host::install_match(&mut world, &plan)
                            .unwrap_or_else(|error| panic!("{selection:?}: {error}"));
                        let expected_units: u32 = [faction, opponent]
                            .iter()
                            .flat_map(|&f| &catalogue.skirmish.factions[f].units)
                            .map(|group| group.count)
                            .sum();
                        assert_eq!(start.units, expected_units as usize);
                        let host = world.resource::<SimHost>();
                        assert!(host.running());
                        assert_eq!(
                            host.ai().map(|ai| ai.players().collect::<Vec<_>>()),
                            Some(vec![OPPONENT])
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn menu_buttons_cycle_through_each_row_and_start_only_a_valid_choice() {
        let start = MenuSelection::default();
        // Next and previous wrap around, row by row.
        let next = |selection, row| match menu_effect(MenuButton::Next(row), selection, COUNTS) {
            MenuEffect::Select(next) => next,
            other => panic!("{other:?}"),
        };
        let one = next(start, Row::Map);
        assert_eq!(one, MenuSelection { map: 1, ..start });
        assert_eq!(next(one, Row::Map), start, "wraps");
        assert_eq!(
            menu_effect(MenuButton::Previous(Row::Faction), start, COUNTS),
            MenuEffect::Select(MenuSelection {
                faction: 2,
                ..start
            })
        );
        assert_eq!(
            menu_effect(MenuButton::Next(Row::Opponent), start, COUNTS),
            MenuEffect::Select(MenuSelection {
                opponent: 1,
                ..start
            })
        );
        assert_eq!(
            menu_effect(MenuButton::Previous(Row::Difficulty), start, COUNTS),
            MenuEffect::Select(MenuSelection {
                difficulty: 2,
                ..start
            })
        );
        assert_eq!(
            menu_effect(MenuButton::Start, start, COUNTS),
            MenuEffect::Start(start)
        );
        assert_eq!(
            menu_effect(MenuButton::Quit, start, COUNTS),
            MenuEffect::Quit
        );
        // A selection outside the catalogue starts nothing; an empty row selects nothing.
        let outside = MenuSelection { map: 2, ..start };
        assert_eq!(
            menu_effect(MenuButton::Start, outside, COUNTS),
            MenuEffect::Nothing
        );
        let empty = MenuCounts {
            difficulties: 0,
            ..COUNTS
        };
        assert_eq!(
            menu_effect(MenuButton::Next(Row::Difficulty), start, empty),
            MenuEffect::Nothing
        );
        assert_eq!(
            menu_effect(MenuButton::Start, start, empty),
            MenuEffect::Nothing
        );
    }

    #[test]
    fn the_human_plays_the_first_start_and_the_computer_the_second() {
        let selection = MenuSelection {
            map: 0,
            faction: 1,
            opponent: 2,
            difficulty: 1,
        };
        assert_eq!(
            selection.players(),
            [
                PlayerSetup {
                    player: PlayerId(0),
                    faction: 1,
                    start: 0,
                    controller: Controller::Human,
                },
                PlayerSetup {
                    player: PlayerId(1),
                    faction: 2,
                    start: 1,
                    controller: Controller::Ai { difficulty: 1 },
                },
            ]
        );
    }

    #[test]
    fn the_end_screen_names_the_outcome_and_restarts_the_last_match() {
        assert_eq!(end_text(None), None);
        assert_eq!(
            end_text(Some(MatchOutcome::Won {
                winner: HUMAN,
                tick: 5
            })),
            Some("VICTORY")
        );
        assert_eq!(
            end_text(Some(MatchOutcome::Won {
                winner: OPPONENT,
                tick: 5
            })),
            Some("DEFEAT")
        );
        assert_eq!(end_text(Some(MatchOutcome::Draw { tick: 5 })), Some("DRAW"));
        let last = MenuSelection {
            map: 1,
            faction: 0,
            opponent: 1,
            difficulty: 2,
        };
        assert_eq!(
            end_effect(EndButton::Restart, Some(last)),
            EndEffect::Restart(last)
        );
        assert_eq!(end_effect(EndButton::Menu, Some(last)), EndEffect::Menu);
        assert_eq!(end_effect(EndButton::Restart, None), EndEffect::Menu);
    }

    #[test]
    fn row_texts_name_the_chosen_entries() {
        let catalogue = production_catalogue();
        let selection = MenuSelection::initial(&catalogue);
        assert_eq!(
            row_text(Row::Map, selection, &catalogue),
            catalogue.maps[0].id.replace('_', " ")
        );
        assert_eq!(
            row_text(Row::Difficulty, selection, &catalogue),
            catalogue.ai.difficulties[selection.difficulty]
                .id
                .replace('_', " ")
        );
        let outside = MenuSelection {
            map: catalogue.maps.len(),
            ..selection
        };
        assert_eq!(row_text(Row::Map, outside, &catalogue), "-");
    }
}
