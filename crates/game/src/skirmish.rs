//! Game flow (PLAN.md section 7, M6): the catalogue of maps, factions, and difficulties loaded at
//! startup, the main menu with map, faction, and difficulty selection, the start of a match against
//! the computer player, the end screen with the winner, and restart (docs/DECISIONS.md D44).
//!
//! When the game starts, [`load_catalogue`] loads and checks everything a match can use: the unit,
//! economy, and combat files with every sprite (`crate::units::load_match_data`), the skirmish file
//! (factions, maps, start positions, starting credits) and the AI file (`sim::load_skirmish`,
//! `sim::load_ai`), and every map of the skirmish file with its tileset image
//! (`crate::map::check_map`). The files are then checked against each other ([`check_catalogue`]):
//! the start positions against their maps, the AI file against the factions, and every match the
//! menu can choose is set up once (M6 review 1 P1-1 and P2-1). The renderer receives only the
//! checked bytes, which are registered once (D16); nothing is read again when a match starts or
//! restarts. Any error ends the game with exit code 1, as before M6.
//!
//! The menu chooses a map, the human player's faction, the computer player's faction, and the
//! difficulty ([`menu_effect`], pure). Starting a match ([`begin_match`]) ends the current one if
//! any ([`end_match`]: the map, the unit and building views, and every resource of the match are
//! removed or reset), sets up a new simulation with `sim::setup_match` through
//! `crate::sim_host::install_match` (human player 0 at the map's first start position, the
//! computer player 1 at the second), spawns the map, sizes the minimap, and centres the camera on
//! the human player's start.
//!
//! Every map can be played by 2 to `sim::MAX_MATCH_PLAYERS` players. The map of a match grows
//! with them ([`match_map`], `sim::enlarged_map`): two players play on the map file as it is,
//! every further player adds an eighth to its width and height, and the start positions the file
//! lacks are made on the enlarged map. The enlargement depends only on the map and the number of
//! players, so a restart plays on the same map. [`match_plan`] is the one place that turns the
//! menu's choices into what the simulation is set up from.
//!
//! The seed of the simulation is taken from the clock when the match
//! starts and logged (M0 debt item 3). When the simulation reports the match's outcome, ticks stop
//! and the end screen shows victory, defeat, or a draw, with Restart (the same choices, a new seed)
//! and Main menu ([`end_effect`], pure). Since M7 the menu also opens the options screen
//! (`crate::options`, docs/DECISIONS.md D52).

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
use crate::hotkeys::ControlGroups;
use crate::map::{CheckedMap, MapView, check_map};
use crate::projection::{cell_centre, map_bounds};
use crate::selection::{DragBox, Selection};
use crate::sim_host::{MatchPlan, SimHost};
use crate::units::{HUMAN, UnitSprites, UnitStatus, UnitView, UnitViews};

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
            .add_systems(
                Startup,
                spawn_screens
                    .after(crate::sidebar::spawn_sidebar)
                    .after(load_catalogue),
            )
            .add_systems(
                Update,
                (screen_input, update_screens, update_player_roster)
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
    let catalogue = Catalogue {
        definitions,
        skirmish,
        ai,
        maps,
    };
    check_catalogue(&catalogue, root)?;
    Ok((catalogue, loaded.sheets, buildings))
}

/// Checks the files of the catalogue against each other, so that every match the menu offers can
/// start and its computer player can play (M6 review 1 P1-1 and P2-1):
///
/// * every start position of every map is a cell of the map with passable terrain
///   (`sim::check_map_starts`);
/// * the AI file can be played by every faction: the producers of the buildings it builds are
///   starting buildings or are built before (`sim::check_ai_factions`);
/// * every match the menu can choose can be set up: every map, enlarged for 2 to
///   `sim::MAX_MATCH_PLAYERS` players ([`match_map`]), each in a simulation of its own that is
///   dropped afterwards. The fewest and the most players are set up with every faction for the
///   human player and every faction for the computer players; the player counts between them with
///   one pairing each, which changes with the count, so that the game still starts quickly (the
///   difficulty does not change the setup).
///
/// Each error names the file and the field, or the map and the factions, and the reason.
pub fn check_catalogue(catalogue: &Catalogue, root: &Path) -> Result<(), String> {
    let skirmish_path = root.join(SKIRMISH_PATH);
    for map in 0..catalogue.maps.len() {
        sim::check_map_starts(
            &skirmish_path,
            &catalogue.skirmish,
            map,
            &catalogue.maps[map].map.grid,
        )
        .map_err(|error| error.to_string())?;
    }
    sim::check_ai_factions(
        root.join(AI_PATH),
        &catalogue.ai,
        &catalogue.skirmish,
        &catalogue.definitions,
    )
    .map_err(|error| error.to_string())?;
    let factions = &catalogue.skirmish.factions;
    let counts = MenuCounts::of(catalogue);
    for (map, entry) in catalogue.maps.iter().enumerate() {
        let most = counts.of_row(Row::Enemies, map).saturating_sub(1);
        for enemies in 0..=most {
            let cannot = |faction: usize, opponent: usize, error: String| {
                format!(
                    "data file {}: map {:?}: the match of faction {:?} against faction {:?} \
                     with {} computer players cannot be set up: {error}",
                    skirmish_path.display(),
                    entry.id,
                    factions
                        .get(faction)
                        .map_or("?", |faction| faction.id.as_str()),
                    factions
                        .get(opponent)
                        .map_or("?", |faction| faction.id.as_str()),
                    enemies + 1,
                )
            };
            // The map for this many players is the same for every pairing: enlarged once.
            let played =
                match_map(catalogue, map, enemies + 2).map_err(|error| cannot(0, 0, error))?;
            // Every pairing with one opponent and with the most; one in between.
            let pairings: Vec<(usize, usize)> = if enemies == 0 || enemies == most {
                (0..factions.len())
                    .flat_map(|faction| (0..factions.len()).map(move |other| (faction, other)))
                    .collect()
            } else {
                let count = factions.len().max(1);
                vec![(enemies % count, (enemies / count) % count)]
            };
            for (faction, opponent) in pairings {
                let selection = MenuSelection {
                    map,
                    faction,
                    opponent,
                    enemies,
                    ..MenuSelection::default()
                };
                crate::sim_host::check_match(&plan_on(catalogue, selection, 0, &played))
                    .map_err(|error| cannot(faction, opponent, error))?;
            }
        }
    }
    Ok(())
}

/// The map of a match: a map of the catalogue as its file defines it, or enlarged for the
/// match's players.
#[derive(Clone, Debug)]
pub struct MatchMap {
    /// The terrain, looks and size the match is played and drawn with.
    pub map: CheckedMap,
    /// The start positions on it, the human player's first: at least one per player.
    pub starts: Vec<sim::CellPos>,
    /// Whether it is the map file's own map, cell for cell (a match of two players).
    pub as_filed: bool,
}

/// The size in cells of map `map` of the catalogue in a match with `players` players
/// (`sim::enlarged_size`), for the menu.
pub fn match_map_size(catalogue: &Catalogue, map: usize, players: usize) -> Option<(u32, u32)> {
    let view = catalogue.maps.get(map)?.map.view;
    Some(sim::enlarged_size(view.width, view.height, players))
}

/// Map `map` of the catalogue for a match with `players` players (`sim::enlarged_map`): the map
/// file's own for two, and one an eighth wider and higher for every further player, with the
/// file's start positions moved along and the missing ones made. A pure function of the map and
/// the player count, so every match with these choices, and every restart, has the same map.
pub fn match_map(catalogue: &Catalogue, map: usize, players: usize) -> Result<MatchMap, String> {
    let (Some(entry), Some(def)) = (catalogue.maps.get(map), catalogue.skirmish.maps.get(map))
    else {
        return Err(format!("map {map} is not in the catalogue"));
    };
    let enlarged = sim::enlarged_map(&entry.map.grid, &entry.map.looks, &def.starts, players)
        .map_err(|error| error.to_string())?;
    let as_filed = enlarged.grid == entry.map.grid && enlarged.looks == entry.map.looks;
    Ok(MatchMap {
        map: CheckedMap {
            view: MapView {
                width: enlarged.grid.width(),
                height: enlarged.grid.height(),
                tile: entry.map.view.tile,
            },
            grid: enlarged.grid,
            looks: enlarged.looks,
        },
        starts: enlarged.starts,
        as_filed,
    })
}

/// Everything the match of `selection` is set up from, with the seed `seed`: the menu's players
/// on the map enlarged for them ([`match_map`]), and the skirmish file with that map's start
/// positions in place of the file's, so that match setup and the computer players use the cells
/// the bases really stand on. Also returns the map, for drawing it.
pub fn match_plan(
    catalogue: &Catalogue,
    selection: MenuSelection,
    seed: u64,
) -> Result<(MatchPlan, MatchMap), String> {
    let map = match_map(catalogue, selection.map, selection.players().len())?;
    Ok((plan_on(catalogue, selection, seed, &map), map))
}

/// [`match_plan`] on `map`, the map of `selection`'s map for its number of players
/// ([`match_map`]).
fn plan_on(
    catalogue: &Catalogue,
    selection: MenuSelection,
    seed: u64,
    map: &MatchMap,
) -> MatchPlan {
    let mut skirmish = catalogue.skirmish.clone();
    if let Some(def) = skirmish.maps.get_mut(selection.map) {
        def.starts.clone_from(&map.starts);
    }
    MatchPlan {
        super_me: selection.super_me,
        unlock_all: selection.unlock_all,
        unit_speed_percent: selection.unit_speed_percent(),
        grid: map.map.grid.clone(),
        looks: map.map.looks.clone(),
        definitions: catalogue.definitions.clone(),
        skirmish,
        ai: catalogue.ai.clone(),
        map: selection.map,
        players: selection.players(),
        seed,
    }
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
    let mut sprites = BuildingSprites::default();
    for sprite in buildings {
        debug_assert!(!sprite.bytes.is_empty());
        let def = catalogue
            .definitions
            .building(sprite.kind)
            .expect("checked building kind");
        let bytes = match crate::hd_art::checked(
            &root,
            "buildings",
            &def.id,
            crate::hd_art::building_size(def.width, def.height),
        ) {
            Ok(bytes) => bytes,
            Err(error) => {
                error!("{error}");
                world.write_message(AppExit::error());
                return;
            }
        };
        let pair =
            crate::presentation::sprite_pair(&bytes, &mut world.resource_mut::<Assets<Image>>());
        match pair {
            Ok((human, enemy)) => {
                sprites.0.insert(sprite.kind, human);
                sprites.1.insert(sprite.kind, enemy);
            }
            Err(error) => {
                error!("checked sprite {}: {error}", sprite.asset_path.display());
                world.write_message(AppExit::error());
                return;
            }
        }
    }
    world.insert_resource(sprites);
    let mut sprites = UnitSprites::default();
    for sheet in sheets {
        debug_assert!(!sheet.bytes.is_empty());
        let def = catalogue
            .definitions
            .unit(sheet.kind)
            .expect("checked unit kind");
        let bytes = match crate::hd_art::checked(
            &root,
            "units",
            &def.id,
            Vec2::new(crate::hd_art::UNIT_SIZE * 8.0, crate::hd_art::UNIT_SIZE),
        ) {
            Ok(bytes) => bytes,
            Err(error) => {
                error!("{error}");
                world.write_message(AppExit::error());
                return;
            }
        };
        let pair =
            crate::presentation::sprite_pair(&bytes, &mut world.resource_mut::<Assets<Image>>());
        match pair {
            Ok((human, enemy)) => {
                sprites.0.insert(sheet.kind, human);
                sprites.1.insert(sheet.kind, enemy);
            }
            Err(error) => {
                error!("checked sprite {}: {error}", sheet.asset_path.display());
                world.write_message(AppExit::error());
                return;
            }
        }
    }
    world.insert_resource(sprites);
    // Walk cycles: only the types that go on foot have one.
    let mut walks = crate::units::WalkSprites::default();
    for def in catalogue.definitions.units() {
        let Some(kind) = catalogue.definitions.unit_kind(&def.id) else {
            continue;
        };
        if !root
            .join("sprites/hd/walk")
            .join(format!("{}.png", def.id))
            .is_file()
        {
            continue;
        }
        let size = Vec2::new(
            crate::hd_art::WALK_SIZE * 8.0,
            crate::hd_art::WALK_SIZE * crate::hd_art::WALK_FRAMES as f32,
        );
        let pair = crate::hd_art::checked(&root, "walk", &def.id, size).and_then(|bytes| {
            crate::presentation::sprite_pair(&bytes, &mut world.resource_mut::<Assets<Image>>())
        });
        match pair {
            Ok(pair) => {
                walks.0.insert(kind, pair);
            }
            Err(error) => {
                error!("walk cycle of {}: {error}", def.id);
                world.write_message(AppExit::error());
                return;
            }
        }
    }
    world.insert_resource(walks);
    let selection = MenuSelection::opening(&catalogue);
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
    /// How many computer players (1 to `sim::MAX_MATCH_PLAYERS` minus one, on every map: the map
    /// grows with its players).
    Enemies,
    Opponent,
    Difficulty,
    SuperMe,
    /// Standard tech tree, or every prerequisite waived for the human player.
    UnlockAll,
    Color,
    /// How fast the match runs ([`crate::game_speed::SPEED_LEVELS`]); `[` and `]` and the
    /// sidebar change it during the match.
    GameSpeed,
    /// How fast the human player's units move with Super Me ([`UNIT_SPEEDS`]).
    UnitSpeed,
}

/// The unit speeds the Super Me player may choose, in percent of every unit type's speed.
pub const UNIT_SPEEDS: [u16; 5] = [100, 150, 200, 300, 400];

impl Row {
    pub const ALL: [Row; 10] = [
        Row::Map,
        Row::Faction,
        Row::Enemies,
        Row::Opponent,
        Row::Difficulty,
        Row::GameSpeed,
        Row::SuperMe,
        Row::UnitSpeed,
        Row::UnlockAll,
        Row::Color,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Row::Map => "Map",
            Row::Faction => "Starting force",
            Row::Enemies => "Enemy players",
            Row::Opponent => "Enemy force",
            Row::Difficulty => "Difficulty",
            Row::SuperMe => "Super Me",
            Row::UnlockAll => "Tech tree",
            Row::Color => "Team color",
            Row::GameSpeed => "Game speed",
            Row::UnitSpeed => "Unit speed",
        }
    }
}

/// The choices of a match: indices into the catalogue's maps, factions, and difficulties.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuSelection {
    pub super_me: bool,
    /// Every building prerequisite waived for the human player (match option, like Super Me).
    pub unlock_all: bool,
    /// Computer players minus one (0: the classic one-against-one).
    pub enemies: usize,
    pub color: usize,
    pub map: usize,
    pub faction: usize,
    pub opponent: usize,
    pub difficulty: usize,
    /// Index into [`crate::game_speed::SPEED_LEVELS`].
    pub game_speed: usize,
    /// Index into [`UNIT_SPEEDS`]; it takes effect with Super Me only.
    pub unit_speed: usize,
}

impl Default for MenuSelection {
    fn default() -> Self {
        Self {
            super_me: false,
            unlock_all: false,
            enemies: 0,
            color: 0,
            map: 0,
            faction: 0,
            opponent: 0,
            difficulty: 0,
            game_speed: crate::game_speed::NORMAL_SPEED,
            unit_speed: 0,
        }
    }
}

/// How many entries each row has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuCounts {
    pub maps: usize,
    pub factions: usize,
    pub difficulties: usize,
    /// Per map, the most computer players a match on it may have: `sim::MAX_MATCH_PLAYERS`
    /// minus one on every map of the catalogue, however many start positions its file lists (the
    /// map of a match is enlarged for its players and gets the missing ones, [`match_map`]).
    pub enemies: [usize; sim::MAX_MAPS],
}

impl MenuCounts {
    pub fn of(catalogue: &Catalogue) -> Self {
        let mut enemies = [0; sim::MAX_MAPS];
        for slot in enemies.iter_mut().take(catalogue.maps.len()) {
            *slot = sim::MAX_MATCH_PLAYERS - 1;
        }
        Self {
            maps: catalogue.maps.len(),
            factions: catalogue.skirmish.factions.len(),
            difficulties: catalogue.ai.difficulties.len(),
            enemies,
        }
    }

    /// How many entries `row` has with map `map` chosen.
    pub fn of_row(self, row: Row, map: usize) -> usize {
        match row {
            Row::Map => self.maps,
            Row::Faction | Row::Opponent => self.factions,
            Row::Enemies => self.enemies.get(map).copied().unwrap_or(0),
            Row::Difficulty => self.difficulties,
            Row::SuperMe | Row::UnlockAll => 2,
            Row::Color => crate::presentation::TEAM_PALETTE.len(),
            Row::GameSpeed => crate::game_speed::SPEED_LEVELS.len(),
            Row::UnitSpeed => UNIT_SPEEDS.len(),
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

    /// What the menu shows when the game starts: [`MenuSelection::initial`] on the first of the
    /// largest maps of the catalogue, where bases lie farthest apart.
    pub fn opening(catalogue: &Catalogue) -> Self {
        let cells = |entry: &CatalogueMap| {
            u64::from(entry.map.grid.width()) * u64::from(entry.map.grid.height())
        };
        let largest = catalogue.maps.iter().map(cells).max().unwrap_or(0);
        Self {
            map: catalogue
                .maps
                .iter()
                .position(|entry| cells(entry) == largest)
                .unwrap_or(0),
            ..Self::initial(catalogue)
        }
    }

    fn get(self, row: Row) -> usize {
        match row {
            Row::Map => self.map,
            Row::Faction => self.faction,
            Row::Enemies => self.enemies,
            Row::Opponent => self.opponent,
            Row::Difficulty => self.difficulty,
            Row::SuperMe => usize::from(self.super_me),
            Row::UnlockAll => usize::from(self.unlock_all),
            Row::Color => self.color,
            Row::GameSpeed => self.game_speed,
            Row::UnitSpeed => self.unit_speed,
        }
    }

    /// The speed of the human player's units in percent: the chosen unit speed with Super Me,
    /// else 100.
    pub fn unit_speed_percent(self) -> u16 {
        if self.super_me {
            UNIT_SPEEDS.get(self.unit_speed).copied().unwrap_or(100)
        } else {
            100
        }
    }

    fn set(&mut self, row: Row, value: usize) {
        match row {
            Row::Map => self.map = value,
            Row::Faction => self.faction = value,
            Row::Enemies => self.enemies = value,
            Row::Opponent => self.opponent = value,
            Row::Difficulty => self.difficulty = value,
            Row::SuperMe => self.super_me = value != 0,
            Row::UnlockAll => self.unlock_all = value != 0,
            Row::Color => self.color = value,
            Row::GameSpeed => self.game_speed = value,
            Row::UnitSpeed => self.unit_speed = value,
        }
    }

    /// Whether every choice names an entry of the catalogue.
    pub fn valid(self, counts: MenuCounts) -> bool {
        Row::ALL
            .into_iter()
            .all(|row| self.get(row) < counts.of_row(row, self.map))
    }

    /// How many computer players the match has.
    pub fn enemy_count(self) -> usize {
        self.enemies + 1
    }

    /// The players of the match: the human player at the first start position of the match's
    /// map ([`match_map`]), computer player k (`PlayerId(k)`, from 1) at start position k. Each computer player is independent:
    /// its own id, base, credits, queues, AI state and colour; all of them are hostile to every
    /// other player.
    pub fn players(self) -> Vec<PlayerSetup> {
        let mut players = vec![PlayerSetup {
            player: HUMAN,
            faction: self.faction,
            start: 0,
            controller: Controller::Human,
        }];
        players.extend((1..=self.enemy_count()).map(|k| PlayerSetup {
            player: sim::PlayerId(u8::try_from(k).unwrap_or(u8::MAX)),
            faction: self.opponent,
            start: k,
            controller: Controller::Ai {
                difficulty: self.difficulty,
            },
        }));
        players
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
    /// The options screen (M7, `crate::options`).
    Options,
    Quit,
}

/// What a menu button does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuEffect {
    Select(MenuSelection),
    Start(MenuSelection),
    /// Open the options screen.
    Options,
    Quit,
    Nothing,
}

/// The effect of `button` with the current `selection` and a catalogue of `counts` entries. A
/// selection outside the catalogue (which the buttons cannot make) starts nothing.
pub fn menu_effect(button: MenuButton, selection: MenuSelection, counts: MenuCounts) -> MenuEffect {
    let step = |row: Row, forward: bool| {
        let count = counts.of_row(row, selection.map);
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
        // A map that admits fewer enemies keeps as many as it can hold.
        next.enemies = next
            .enemies
            .min(counts.of_row(Row::Enemies, next.map).saturating_sub(1));
        MenuEffect::Select(next)
    };
    match button {
        MenuButton::Previous(row) => step(row, false),
        MenuButton::Next(row) => step(row, true),
        MenuButton::Start if selection.valid(counts) => MenuEffect::Start(selection),
        MenuButton::Start => MenuEffect::Nothing,
        MenuButton::Options => MenuEffect::Options,
        MenuButton::Quit => MenuEffect::Quit,
    }
}

/// The text of a menu row: the id of the chosen entry, with underscores as spaces. The map row
/// adds the size in cells the map has for the chosen number of players, `liberty bay  176 x 176`,
/// which changes with the enemy players row ([`match_map_size`]).
pub fn row_text(row: Row, selection: MenuSelection, catalogue: &Catalogue) -> String {
    let id = match row {
        Row::SuperMe => Some(if selection.super_me {
            "ON / 10x / $1"
        } else {
            "OFF"
        }),
        Row::UnlockAll => Some(if selection.unlock_all {
            "ALL UNLOCKED"
        } else {
            "STANDARD"
        }),
        Row::GameSpeed => {
            return crate::game_speed::SPEED_LEVELS
                .get(selection.game_speed)
                .map_or_else(|| "-".to_string(), |&f| crate::game_speed::speed_label(f));
        }
        Row::UnitSpeed => {
            let Some(&percent) = UNIT_SPEEDS.get(selection.unit_speed) else {
                return "-".to_string();
            };
            let label = crate::game_speed::speed_label(f32::from(percent) / 100.0);
            return if selection.super_me {
                label
            } else {
                format!("{label} - SUPER ME OFF")
            };
        }
        Row::Enemies => {
            let most = MenuCounts::of(catalogue).of_row(Row::Enemies, selection.map);
            return format!("{} of {most}", selection.enemy_count());
        }
        Row::Color => crate::presentation::TEAM_PALETTE
            .get(selection.color)
            .map(|c| c.0),
        Row::Map => {
            let size = match_map_size(catalogue, selection.map, selection.players().len());
            return match (catalogue.maps.get(selection.map), size) {
                (Some(map), Some((width, height))) => {
                    format!("{}  {width} x {height}", map.id.replace('_', " "))
                }
                _ => "-".to_string(),
            };
        }
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

/// The headline of the end screen for the human player, or `None` while the match goes on. With
/// several computer players the simulation's outcome waits for the last one standing; the human
/// player's defeat (`human_defeated`) ends the match for the human at once.
pub fn end_text(outcome: Option<MatchOutcome>, human_defeated: bool) -> Option<&'static str> {
    if outcome.is_none() && human_defeated {
        return Some("DEFEAT");
    }
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
/// ([`end_match`]), sets up the simulation on the map enlarged for the match's players
/// ([`match_plan`]), spawns that map, sizes the minimap and the camera's bounds for it, and
/// centres the camera on the human player's start. A setup error is logged and ends the game with
/// exit code 1, as before M6; nothing of a new match is then shown.
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
    let (plan, map) = match match_plan(&catalogue, selection, seed) {
        Ok(planned) => planned,
        Err(error) => {
            error!("map {}: {error}", entry.id);
            world.write_message(AppExit::error());
            return;
        }
    };
    let start = match crate::sim_host::install_match(world, &plan) {
        Ok(start) => start,
        Err(error) => {
            error!("map {}: {error}", entry.id);
            world.write_message(AppExit::error());
            return;
        }
    };
    if let Some(mut speed) = world.get_resource_mut::<crate::game_speed::GameSpeed>() {
        speed.set_level(selection.game_speed);
    }
    let faction = |index: usize| {
        catalogue
            .skirmish
            .factions
            .get(index)
            .map_or("?", |faction| faction.id.as_str())
    };
    info!(
        "match on map {} ({} x {} cells, seed {seed}): {} against {} x {} ({}){}{}: {} buildings \
         and {} units spawned",
        entry.id,
        map.map.view.width,
        map.map.view.height,
        faction(selection.faction),
        selection.enemy_count(),
        faction(selection.opponent),
        catalogue
            .ai
            .difficulties
            .get(selection.difficulty)
            .map_or("?", |difficulty| difficulty.id.as_str()),
        if selection.super_me { ", Super Me" } else { "" },
        if selection.unlock_all {
            ", all unlocked"
        } else {
            ""
        },
        start.buildings,
        start.units
    );
    world.resource_mut::<BuildingStatus>().spawned = start.buildings;
    world.resource_mut::<UnitStatus>().spawned = start.units;
    let view = map.map.view;
    crate::map::spawn_map(
        world,
        &entry.asset_path,
        &map.map,
        &map.starts,
        map.as_filed,
    );
    world.insert_resource(CameraBounds(Some(map_bounds(
        view.width,
        view.height,
        view.tile,
    ))));
    crate::minimap::size_minimap(world, view.width, view.height);
    if let Some(&start) = map.starts.first() {
        let centre = cell_centre(sim::CellPos::new(start.x + 3, start.y + 3), view.tile);
        let cameras: Vec<Entity> = world
            .query_filtered::<Entity, With<Camera2d>>()
            .iter(world)
            .collect();
        for camera in cameras {
            if let Some(mut transform) = world.get_mut::<Transform>(camera) {
                transform.translation = (centre + Vec2::X * crate::sidebar::SIDEBAR_WIDTH / 2.0)
                    .extend(transform.translation.z);
            }
        }
    }
    let mut pairs = Vec::new();
    if let Some(walks) = world.get_resource::<crate::units::WalkSprites>() {
        pairs.extend(walks.0.values().cloned());
    }
    if let Some(s) = world.get_resource::<UnitSprites>() {
        pairs.extend(
            s.0.iter()
                .filter_map(|(k, h)| s.1.get(k).map(|e| (h.clone(), e.clone()))),
        );
    }
    if let Some(s) = world.get_resource::<BuildingSprites>() {
        pairs.extend(
            s.0.iter()
                .filter_map(|(k, h)| s.1.get(k).map(|e| (h.clone(), e.clone()))),
        );
    }
    crate::presentation::apply_team_colors(world, selection.color, pairs);
    world.insert_resource(LastMatch(Some(selection)));
    *world.resource_mut::<Screen>() = Screen::Match;
}

/// Ends the current match, if any, and shows the menu: the simulation is replaced by one without
/// a match, and the map, the unit and building views, and the interface state of the match are
/// removed or reset, so that nothing of it reaches the next match.
pub(crate) fn end_match(world: &mut World) {
    crate::sim_host::uninstall_match(world);
    crate::battlefield::clear(world);
    crate::notices::clear(world);
    crate::map::despawn_map(world);
    let views: Vec<Entity> = world
        .query_filtered::<Entity, Or<(With<UnitView>, With<BuildingView>)>>()
        .iter(world)
        .collect();
    for view in views {
        world.despawn(view);
    }
    world.insert_resource(UnitViews::default());
    world.insert_resource(crate::units::Altitudes::default());
    world.insert_resource(BuildingViews::default());
    world.insert_resource(UnitStatus::default());
    world.insert_resource(BuildingStatus::default());
    world.insert_resource(Selection::default());
    world.insert_resource(crate::selection::SelectedBuilding::default());
    world.insert_resource(DragBox::default());
    world.insert_resource(ControlGroups::default());
    world.insert_resource(InterfaceMode::default());
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
    /// A line of the menu's player list: player `n` of the match (0 the human player).
    Player(usize),
}

/// A line of the menu's player list, shown for the players of the chosen match.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerLine(pub usize);

/// The colour swatch of a line of the menu's player list.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerSwatch(pub usize);

/// The text of line `slot` of the player list: `You - Blue - vanguard` or
/// `AI 2 - Gold - meridian, normal` (ASCII: the interface font has no middle dot); `None` beyond the match's players.
pub fn player_line(slot: usize, selection: MenuSelection, catalogue: &Catalogue) -> Option<String> {
    let setup = selection.players().into_iter().nth(slot)?;
    let colour = crate::presentation::TEAM_PALETTE
        [crate::presentation::team_color_index(selection.color, setup.player)]
    .0;
    let force = catalogue
        .skirmish
        .factions
        .get(setup.faction)
        .map_or("-", |faction| faction.id.as_str());
    Some(match setup.controller {
        Controller::Human => format!("You - {colour} - {force}"),
        Controller::Ai { difficulty } => format!(
            "AI {slot} - {colour} - {force}, {}",
            catalogue
                .ai
                .difficulties
                .get(difficulty)
                .map_or("-", |difficulty| difficulty.id.as_str())
        ),
    })
}

/// Shows the menu's player list for the current choices.
fn update_player_roster(
    selection: Res<MenuSelection>,
    catalogue: Option<Res<Catalogue>>,
    mut lines: Query<(&PlayerLine, &mut Node)>,
    mut swatches: Query<(&PlayerSwatch, &mut BackgroundColor)>,
) {
    if !selection.is_changed() && !catalogue.as_ref().is_some_and(|c| c.is_changed()) {
        return;
    }
    let count = selection.players().len();
    for (line, mut node) in &mut lines {
        let display = if line.0 < count {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
    for (swatch, mut colour) in &mut swatches {
        let rgb = crate::presentation::TEAM_PALETTE[crate::presentation::team_color_index(
            selection.color,
            sim::PlayerId(u8::try_from(swatch.0).unwrap_or(0)),
        )]
        .1;
        colour.0 = Color::srgb_u8(rgb[0], rgb[1], rgb[2]);
    }
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
            border: UiRect::all(Val::Px(1.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BorderColor::all(Color::srgb(0.43, 0.46, 0.32)),
        BackgroundColor(Color::srgb(0.16, 0.21, 0.20)),
    )
}

/// Width of the chosen entry of a menu row, in logical pixels.
const ROW_VALUE_WIDTH: f32 = 320.0;

/// Width of the menu's player list, in logical pixels.
const PLAYERS_PANEL_WIDTH: f32 = 330.0;

/// Spawns the menu (shown at first) and the end screen (hidden), above the sidebar. The player
/// list has a line for each of the `sim::MAX_MATCH_PLAYERS` players a match may have; ten lines
/// are lower than the rows beside them.
pub(crate) fn spawn_screens(mut commands: Commands, sprites: Option<Res<BuildingSprites>>) {
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
            BackgroundColor(Color::srgb(0.055, 0.075, 0.078)),
            GlobalZIndex(10),
            FocusPolicy::Block,
        ))
        .with_children(|menu| {
            menu.spawn(screen_text("FIELD OPERATIONS", 13.0));
            menu.spawn(screen_text("MyGame001", 44.0));
            menu.spawn(screen_text("SKIRMISH COMMAND", 13.0));
            menu.spawn((
                Node {
                    width: Val::Px(464.0),
                    height: Val::Px(2.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.55, 0.47, 0.25)),
            ));
            if let Some(sprites) = &sprites {
                menu.spawn(Node {
                    column_gap: Val::Px(12.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|gallery| {
                    for kind in [0, 1, 4, 5] {
                        if let Some(image) = sprites.0.get(&sim::BuildingKind(kind)) {
                            gallery.spawn((
                                ImageNode::new(image.clone()),
                                Node {
                                    width: Val::Px(100.0),
                                    height: Val::Px(68.0),
                                    ..default()
                                },
                            ));
                        }
                    }
                });
            }
            menu.spawn(screen_text("SELECT YOUR THEATER OF OPERATIONS", 11.0));
            menu.spawn(Node {
                column_gap: Val::Px(28.0),
                align_items: AlignItems::Start,
                ..default()
            })
            .with_children(|columns| {
                columns
                    .spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        ..default()
                    })
                    .with_children(|rows| {
                        for row in Row::ALL {
                            rows.spawn(Node {
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
                                // Wide enough for the longest map with its size,
                                // `continental divide  256 x 256`.
                                line.spawn((
                                    Node {
                                        width: Val::Px(ROW_VALUE_WIDTH),
                                        justify_content: JustifyContent::Center,
                                        ..default()
                                    },
                                    children![(ScreenText::Row(row), screen_text("", 18.0))],
                                ));
                                line.spawn((MenuButton::Next(row), screen_button(36.0)))
                                    .with_child(screen_text(">", 18.0));
                            });
                        }
                    });
                // Every player of the match with its colour, so the teams are clear before deploying.
                columns
                    .spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(6.0),
                            // Wide enough for the longest line on one row of text, so that
                            // ten players are lower than the rows beside them.
                            width: Val::Px(PLAYERS_PANEL_WIDTH),
                            padding: UiRect::all(Val::Px(10.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BorderColor::all(Color::srgb(0.43, 0.46, 0.32)),
                        BackgroundColor(Color::srgb(0.08, 0.10, 0.10)),
                    ))
                    .with_children(|panel| {
                        panel.spawn(screen_text("PLAYERS", 13.0));
                        for slot in 0..sim::MAX_MATCH_PLAYERS {
                            panel
                                .spawn((
                                    PlayerLine(slot),
                                    Node {
                                        column_gap: Val::Px(8.0),
                                        align_items: AlignItems::Center,
                                        ..default()
                                    },
                                ))
                                .with_children(|line| {
                                    line.spawn((
                                        PlayerSwatch(slot),
                                        Node {
                                            width: Val::Px(16.0),
                                            height: Val::Px(16.0),
                                            border: UiRect::all(Val::Px(1.0)),
                                            ..default()
                                        },
                                        BorderColor::all(Color::srgb(0.9, 0.9, 0.9)),
                                        BackgroundColor(Color::BLACK),
                                    ));
                                    line.spawn((ScreenText::Player(slot), screen_text("", 14.0)));
                                });
                        }
                    });
            });
            menu.spawn((MenuButton::Start, screen_button(200.0)))
                .with_child(screen_text("DEPLOY", 20.0));
            menu.spawn((MenuButton::Options, screen_button(200.0)))
                .with_child(screen_text("Options", 18.0));
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
pub(crate) fn screen_input(
    mut commands: Commands,
    screen: Res<Screen>,
    catalogue: Option<Res<Catalogue>>,
    mut selection: ResMut<MenuSelection>,
    last: Res<LastMatch>,
    host: Res<SimHost>,
    mut options: ResMut<crate::options::OptionsOpen>,
    menu: Query<(&Interaction, &MenuButton), Changed<Interaction>>,
    end: Query<(&Interaction, &EndButton), Changed<Interaction>>,
) {
    match *screen {
        // While the options screen is open, it takes the clicks (`crate::options`).
        Screen::Menu if options.0 => {}
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
                    MenuEffect::Options => options.0 = true,
                    MenuEffect::Quit => {
                        commands.queue(|world: &mut World| {
                            world.write_message(AppExit::Success);
                        });
                    }
                    MenuEffect::Nothing => {}
                }
            }
        }
        Screen::Match if host.outcome().is_some() || host.human_defeated() => {
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
pub(crate) fn update_screens(
    screen: Res<Screen>,
    selection: Res<MenuSelection>,
    catalogue: Option<Res<Catalogue>>,
    host: Res<SimHost>,
    mut roots: Query<(&mut Node, Has<MenuRoot>), Or<(With<MenuRoot>, With<EndRoot>)>>,
    mut texts: Query<(&mut Text, &ScreenText)>,
) {
    let outcome = end_text(host.outcome(), host.human_defeated());
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
            ScreenText::Player(slot) => catalogue
                .as_deref()
                .and_then(|catalogue| player_line(*slot, *selection, catalogue))
                .unwrap_or_default(),
        };
        if text.0 != value {
            text.0 = value;
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    #[test]
    fn menu_bundles_spawn_without_duplicate_components() {
        let mut app = bevy::prelude::App::new();
        app.add_plugins(bevy::prelude::MinimalPlugins)
            .add_systems(bevy::prelude::Startup, super::spawn_screens);
        app.update();
        let mut buttons = app.world_mut().query::<&super::MenuButton>();
        assert_eq!(buttons.iter(app.world()).count(), Row::ALL.len() * 2 + 3);
    }

    use std::collections::BTreeSet;

    use sim::PlayerId;

    use super::*;
    use crate::units::OPPONENT;

    fn assets() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    pub(crate) fn production_catalogue() -> Catalogue {
        load(&assets(), &CheckedAssets::default())
            .unwrap_or_else(|error| panic!("{error}"))
            .0
    }

    /// Every file under `dir`, relative to it, in sorted order.
    fn files_under(dir: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let mut pending = vec![dir.to_path_buf()];
        while let Some(next) = pending.pop() {
            for entry in std::fs::read_dir(&next).expect("directory") {
                let entry = entry.expect("entry");
                if entry.file_type().expect("type").is_dir() {
                    pending.push(entry.path());
                } else {
                    files.push(
                        entry
                            .path()
                            .strip_prefix(dir)
                            .expect("under the directory")
                            .to_path_buf(),
                    );
                }
            }
        }
        files.sort();
        files
    }

    #[test]
    fn every_production_data_file_loads_passes_cross_validation_and_is_licensed() {
        // PLAN.md section 7, M7: "every production data file loads and passes cross validation".
        // The catalogue loads every data file through its subset checker, every map with its
        // tileset image, and every sprite, and checks them against each other (check_catalogue:
        // start positions, the AI file against every faction's roster, and a trial setup of every
        // match the menu offers).
        let root = assets();
        let catalogue = production_catalogue();
        check_catalogue(&catalogue, &root).unwrap_or_else(|error| panic!("{error}"));
        // M7 content: two factions with different rosters, three maps, the full roster.
        let factions = &catalogue.skirmish.factions;
        assert_eq!(factions.len(), 2);
        assert_ne!(factions[0].roster, factions[1].roster);
        assert!(catalogue.maps.len() >= 3);
        let offered: std::collections::BTreeSet<sim::ProductionItem> = factions
            .iter()
            .flat_map(|faction| faction.roster.iter().copied())
            .chain(catalogue.definitions.arsenal().0.keys().copied())
            .collect();
        for item in crate::construction::production_menu(&catalogue.definitions) {
            assert!(offered.contains(&item), "{item:?} is offered by no faction");
        }
        // No data file is left unchecked: the data directory holds exactly the files the
        // catalogue loads.
        let data: Vec<PathBuf> = files_under(&root.join("data"));
        let loaded: Vec<PathBuf> = [
            crate::units::UNITS_PATH,
            crate::buildings::ECONOMY_PATH,
            crate::units::COMBAT_PATH,
            "data/arsenal.ron",
            SKIRMISH_PATH,
            AI_PATH,
        ]
        .iter()
        .map(|path| {
            Path::new(path)
                .strip_prefix("data")
                .expect("data")
                .to_path_buf()
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
        assert_eq!(data, loaded);
        // Every map directory but the M0 spike's is a map of the menu.
        for entry in std::fs::read_dir(root.join("maps")).expect("maps") {
            let name = entry
                .expect("entry")
                .file_name()
                .into_string()
                .expect("name");
            assert!(
                name == "spike" || catalogue.maps.iter().any(|map| map.id == name),
                "maps/{name} is not a map of the skirmish file"
            );
        }
        // Every sprite is named by a definition.
        let definitions = &catalogue.definitions;
        for file in files_under(&root.join("sprites")) {
            let stem = file
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("name");
            let hd_used = if file.starts_with("hd/buildings") {
                definitions.buildings().iter().any(|d| d.id == stem)
            } else if file.starts_with("hd/units") || file.starts_with("hd/walk") {
                // A walk cycle belongs to the unit whose sheet has its name.
                definitions.units().iter().any(|d| d.id == stem)
            } else {
                // The terrain atlases battlefield.rs loads, and the particle textures.
                (0..crate::battlefield::ATLASES)
                    .any(|i| file == Path::new(&format!("hd/terrain/{i}.png")))
                    || ["hd/glow.png", "hd/smoke.png", "hd/ring.png"]
                        .iter()
                        .any(|p| file == Path::new(p))
            };
            let used = hd_used
                || definitions.units().iter().any(|unit| unit.sprite == stem)
                || definitions
                    .buildings()
                    .iter()
                    .any(|building| building.sprite == stem);
            assert!(used, "sprites/{} is named by no definition", file.display());
        }
        // Every file under assets/ is recorded in assets/LICENSES.md (PLAN.md section 3 item 7):
        // the repository generated or authored it, or it carries a recorded licence.
        let licences = std::fs::read_to_string(root.join("LICENSES.md")).expect("LICENSES.md");
        for file in files_under(&root) {
            let name = file.to_str().expect("name");
            if name == "LICENSES.md" {
                continue;
            }
            assert!(
                licences.contains(&format!("`{name}`")),
                "{name} is not recorded in assets/LICENSES.md"
            );
        }
    }

    const COUNTS: MenuCounts = MenuCounts {
        maps: 2,
        factions: 3,
        difficulties: 3,
        enemies: {
            let mut enemies = [0; sim::MAX_MAPS];
            enemies[0] = 1;
            enemies[1] = 3;
            enemies
        },
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
            for &start in &def.starts {
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
                            ..MenuSelection::default()
                        };
                        let mut world = World::new();
                        world.init_resource::<crate::sim_host::CommandSink>();
                        let (plan, played) = match_plan(&catalogue, selection, 0)
                            .unwrap_or_else(|error| panic!("{selection:?}: {error}"));
                        // One against one: the map file's own map and start positions.
                        assert!(played.as_filed, "{selection:?}");
                        assert_eq!(plan.grid, catalogue.maps[map].map.grid);
                        assert_eq!(plan.skirmish, catalogue.skirmish);
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

    /// Copies the files of the directory `from` into `to`, recursively.
    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).expect("directory");
        for entry in std::fs::read_dir(from).expect("readable") {
            let entry = entry.expect("entry");
            let target = to.join(entry.file_name());
            if entry.file_type().expect("type").is_dir() {
                copy_dir(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), &target).expect("copied");
            }
        }
    }

    #[test]
    fn loading_rejects_a_start_position_outside_its_map() {
        // M6 review 1 P1-1, the reviewer's scratch root `p1_start_outside`: the demo map (64 x 64)
        // with starts (27, 37) and (70, 42), and the faction's yard at offset (-12, 0) and groups
        // at (-8, 1) and (-11, 4), so that every building and unit lies on the map but the
        // computer player's start does not. The reviewed code loaded it, and the computer player's
        // commands to (70, 42) were rejected all match long; now the loader rejects it.
        let root = PathBuf::from(std::env::var_os("HOME").expect("HOME for review fixtures"))
            .join("Library/Caches/Codex/reviews/MyGame001/rts-controls")
            .join(format!("skirmish-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for dir in ["data", "maps", "sprites"] {
            copy_dir(&assets().join(dir), &root.join(dir));
        }
        let path = root.join(SKIRMISH_PATH);
        let mut text = std::fs::read_to_string(&path).expect("skirmish file");
        // Since M7 the file has two factions; the edits shift the first faction's (the
        // Vanguard's) yard and groups, as the reviewer's root shifted the only faction's.
        for (from, to) in [
            (
                "(building: \"construction_yard\", x: 0, y: 0)],\n            units: [\n                (unit: \"light_tank\"",
                "(building: \"construction_yard\", x: -12, y: 0)],\n            units: [\n                (unit: \"light_tank\"",
            ),
            (
                "(unit: \"light_tank\", x: 4, y: 1, count: 3)",
                "(unit: \"light_tank\", x: -8, y: 1, count: 3)",
            ),
            (
                "(unit: \"scout\", x: 1, y: 4, count: 2)",
                "(unit: \"scout\", x: -11, y: 4, count: 2)",
            ),
            (
                "(unit: \"rifleman\", x: 4, y: 4, count: 4)",
                "(unit: \"rifleman\", x: -8, y: 4, count: 4)",
            ),
            // The reviewer's root had the two starts of the M6 format; the demo map's later
            // start positions are left out of the copy, like the maps added in M7.
            (
                "starts: [(x: 15, y: 37), (x: 52, y: 42), (x: 4, y: 1), (x: 1, y: 19), \
                 (x: 16, y: 54), (x: 21, y: 2)]",
                "starts: [(x: 27, y: 37), (x: 70, y: 42)]",
            ),
            // The shifted offsets are meant for the demo map only; the maps added in M7 are left
            // out of the copy.
            (
                "        (id: \"crossroads\", starts: [(x: 9, y: 9), (x: 44, y: 44), (x: 3, y: 34), \
                 (x: 50, y: 19), (x: 18, y: 31), (x: 35, y: 22)]),\n",
                "",
            ),
            (
                "        (id: \"riverlands\", starts: [(x: 8, y: 21), (x: 53, y: 24), (x: 5, y: 3), \
                 (x: 56, y: 42)]),\n",
                "",
            ),
        ] {
            assert_eq!(text.matches(from).count(), 1, "{from}");
            text = text.replacen(from, to, 1);
        }
        // Likewise every later map: only the demo map remains in the copy.
        let mut text: String = text
            .lines()
            .filter(|line| {
                !(line.trim_start().starts_with("(id: \"") && !line.contains("(id: \"demo\""))
            })
            .collect::<Vec<_>>()
            .join("\n");
        text.push('\n');
        std::fs::write(&path, text).expect("skirmish file");
        let error = load(&root, &CheckedAssets::default())
            .map(|_| ())
            .expect_err("a start position outside the map");
        for part in [
            format!("data file {}", path.display()),
            "maps[0].starts[1]".to_string(),
            "(70, 42), which is outside the 64 x 64 cells of map \"demo\"".to_string(),
            "a cell of the map with passable terrain".to_string(),
        ] {
            assert!(error.contains(&part), "{error:?} lacks {part:?}");
        }
        // With the computer player's start on the map, the start positions pass. Since maps
        // grow with their players the file is still rejected, by the trial setup of the matches
        // with more players than it lists start positions: a made start position has its clear
        // ground around the start itself (`sim::enlarged_map`), where a faction's yard stands
        // in the shipped data, not twelve cells beside it.
        let shifted = std::fs::read_to_string(&path)
            .expect("skirmish file")
            .replacen("(x: 70, y: 42)", "(x: 58, y: 42)", 1);
        std::fs::write(&path, &shifted).expect("skirmish file");
        let error = load(&root, &CheckedAssets::default())
            .map(|_| ())
            .expect_err("a yard far from a made start");
        for part in [
            "map \"demo\": the match of faction",
            "computer players cannot be set up",
            "starting building \"construction_yard\"",
        ] {
            assert!(error.contains(part), "{error:?} lacks {part:?}");
        }
        // The same files with the yard and the groups back at the start load.
        let mut unshifted = shifted;
        for (from, to) in [
            ("x: -12, y: 0)],", "x: 0, y: 0)],"),
            ("x: -8, y: 1, count: 3)", "x: 4, y: 1, count: 3)"),
            ("x: -11, y: 4, count: 2)", "x: 1, y: 4, count: 2)"),
            ("x: -8, y: 4, count: 4)", "x: 4, y: 4, count: 4)"),
        ] {
            assert_eq!(unshifted.matches(from).count(), 1, "{from}");
            unshifted = unshifted.replacen(from, to, 1);
        }
        std::fs::write(&path, unshifted).expect("skirmish file");
        load(&root, &CheckedAssets::default()).unwrap_or_else(|error| panic!("{error}"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_catalogue_rejects_starts_off_their_map_unbuildable_ai_files_and_unplayable_matches() {
        // M6 review 1 P1-1: a start position outside its map was accepted when the catalogue was
        // loaded (the reviewer's scratch root: (70, 42) on the 64 x 64 demo map, with the
        // faction's buildings and units at offsets that lie on the map), and the computer player's
        // commands to it were rejected all match long. The catalogue's cross checks now reject it,
        // and the AI file that no faction can play (P2-1), and any match the menu offers that
        // cannot be set up (implementation debt item 7), when the game starts.
        let catalogue = production_catalogue();
        let root = assets();
        check_catalogue(&catalogue, &root).unwrap_or_else(|error| panic!("{error}"));
        let skirmish_file = format!("data file {}", root.join(SKIRMISH_PATH).display());

        let mut outside = catalogue.clone();
        outside.skirmish.maps[0].starts[1] = sim::CellPos::new(70, 42);
        for building in &mut outside.skirmish.factions[0].buildings {
            building.offset.0 -= 18;
        }
        for group in &mut outside.skirmish.factions[0].units {
            group.offset.0 -= 18;
        }
        let error = check_catalogue(&outside, &root).expect_err("a start outside the map");
        for part in [
            skirmish_file.as_str(),
            "maps[0].starts[1]",
            "(70, 42), which is outside the 64 x 64 cells of map \"demo\"",
        ] {
            assert!(error.contains(part), "{error:?} lacks {part:?}");
        }
        // A start on impassable terrain.
        let mut on_rock = catalogue.clone();
        let rock = (0..64)
            .flat_map(|y| (0..64).map(move |x| sim::CellPos::new(x, y)))
            .find(|&cell| {
                on_rock.maps[0]
                    .map
                    .grid
                    .cell(cell)
                    .is_some_and(|terrain| !terrain.passable)
            })
            .expect("the demo map has impassable terrain");
        on_rock.skirmish.maps[0].starts[0] = rock;
        let error = check_catalogue(&on_rock, &root).expect_err("a start on rock");
        assert!(error.contains("maps[0].starts[0]"), "{error}");
        assert!(error.contains("impassable terrain"), "{error}");

        // An AI file whose build order needs a producer that a faction does not start with.
        let mut no_yard = catalogue.clone();
        let power_plant = no_yard
            .definitions
            .building_kind("power_plant")
            .expect("power plant");
        no_yard.skirmish.factions[0].buildings[0].kind = power_plant;
        let error = check_catalogue(&no_yard, &root).expect_err("no yard");
        for part in [
            format!("data file {}", root.join(AI_PATH).display()),
            "build_order[0]".to_string(),
            "whose producer \"construction_yard\" faction \"vanguard\"".to_string(),
        ] {
            assert!(error.contains(&part), "{error:?} lacks {part:?}");
        }

        // A faction whose buildings do not fit at a start of a map: the match cannot be set up.
        let mut too_far = catalogue.clone();
        too_far.skirmish.factions[0].buildings[0].offset = (32, 0);
        let error = check_catalogue(&too_far, &root).expect_err("off the map");
        for part in [
            skirmish_file.as_str(),
            "map \"demo\": the match of faction \"vanguard\" against faction \"vanguard\"",
            "starting building \"construction_yard\"",
        ] {
            assert!(error.contains(part), "{error:?} lacks {part:?}");
        }
    }

    #[test]
    fn the_menu_chooses_several_independent_enemies_with_distinct_colours() {
        let catalogue = production_catalogue();
        let counts = MenuCounts::of(&catalogue);
        let map = |id: &str| {
            catalogue
                .skirmish
                .maps
                .iter()
                .position(|m| m.id == id)
                .expect(id)
        };
        let (demo, riverlands) = (map("demo"), map("riverlands"));
        // Nine enemies on every map, also on one whose file lists four start positions: the map
        // of a match grows with its players and gets the start positions it lacks.
        assert_eq!(catalogue.skirmish.maps[riverlands].starts.len(), 4);
        for map in 0..counts.maps {
            assert_eq!(counts.of_row(Row::Enemies, map), sim::MAX_MATCH_PLAYERS - 1);
        }
        assert_eq!(counts.of_row(Row::Enemies, counts.maps), 0, "no such map");
        // Next on the row: 1, 2, ... 9, then back to 1.
        let mut selection = MenuSelection {
            map: demo,
            ..MenuSelection::initial(&catalogue)
        };
        assert_eq!(selection.enemy_count(), 1, "one-against-one by default");
        let mut seen = Vec::new();
        for _ in 0..10 {
            seen.push(selection.enemy_count());
            match menu_effect(MenuButton::Next(Row::Enemies), selection, counts) {
                MenuEffect::Select(next) => selection = next,
                other => panic!("{other:?}"),
            }
        }
        assert_eq!(seen, [1, 2, 3, 4, 5, 6, 7, 8, 9, 1]);
        // Nine enemies stay nine on every map the map row passes.
        selection.enemies = 8;
        assert!(selection.valid(counts));
        let mut next = selection;
        while next.map != riverlands {
            next = match menu_effect(MenuButton::Next(Row::Map), next, counts) {
                MenuEffect::Select(next) => next,
                other => panic!("{other:?}"),
            };
            assert!(next.valid(counts), "{next:?}");
        }
        assert_eq!(next.enemy_count(), 9);
        // The map row shows how large the map is for the chosen players, and the match is
        // played on a map of that size with a start position for everyone.
        let demo_side = catalogue.maps[demo].map.view.width;
        assert_eq!(demo_side, 64);
        for (enemies, side) in [(0, 64), (1, 72), (4, 96), (8, 128)] {
            let choice = MenuSelection {
                map: demo,
                enemies,
                ..selection
            };
            assert_eq!(
                row_text(Row::Map, choice, &catalogue),
                format!("demo  {side} x {side}")
            );
            let (plan, played) = match_plan(&catalogue, choice, 0).expect("a plan");
            assert_eq!((plan.grid.width(), plan.grid.height()), (side, side));
            assert_eq!(
                (played.map.view.width, played.map.view.height),
                (side, side)
            );
            assert_eq!(played.map.looks.len(), (side * side) as usize);
            assert_eq!(played.as_filed, enemies == 0);
            assert_eq!(plan.players.len(), enemies + 2);
            assert!(played.starts.len() >= plan.players.len());
            assert_eq!(plan.skirmish.maps[demo].starts, played.starts);
            // The other maps keep the start positions of the file.
            assert_eq!(
                plan.skirmish.maps[riverlands],
                catalogue.skirmish.maps[riverlands]
            );
            // The same choices give the same map: a restart plays on it again.
            let (again, _) = match_plan(&catalogue, choice, 1).expect("a plan");
            assert_eq!((&again.grid, &again.looks), (&plan.grid, &plan.looks));
            let start = crate::sim_host::check_match(&plan).expect("the match sets up");
            assert_eq!(start.buildings, enemies + 2, "a yard each");
        }
        let longest = MenuSelection {
            map: map("continental_divide"),
            enemies: 8,
            ..selection
        };
        assert_eq!(
            row_text(Row::Map, longest, &catalogue),
            "continental divide  256 x 256"
        );
        // Four computer players: their own ids, start positions, and AI controllers.
        selection.enemies = 3;
        selection.color = 6;
        let players = selection.players();
        assert_eq!(players.len(), 5);
        for (k, setup) in players.iter().enumerate() {
            assert_eq!(setup.player, PlayerId(k as u8));
            assert_eq!(setup.start, k);
            assert_eq!(setup.controller == Controller::Human, k == 0);
        }
        // Every player has its own colour, the human's being the chosen one; the list shows them.
        let lines: Vec<String> = (0..sim::MAX_MATCH_PLAYERS)
            .filter_map(|slot| player_line(slot, selection, &catalogue))
            .collect();
        assert_eq!(lines.len(), 5);
        assert!(lines[0].starts_with("You - Orange"), "{lines:?}");
        assert!(lines[1].starts_with("AI 1 - Pink"), "{lines:?}");
        assert!(lines[2].starts_with("AI 2 - Lime"), "{lines:?}");
        let colours: BTreeSet<&str> = lines
            .iter()
            .map(|line| line.split(" - ").nth(1).expect("colour"))
            .collect();
        assert_eq!(colours.len(), 5);
        assert_eq!(row_text(Row::Enemies, selection, &catalogue), "4 of 9");
        // Ten players: a line and a colour of its own for each, whatever the human chose.
        for color in 0..crate::presentation::TEAM_PALETTE.len() {
            let full = MenuSelection {
                enemies: 8,
                color,
                ..selection
            };
            assert_eq!(full.players().len(), sim::MAX_MATCH_PLAYERS);
            let lines: Vec<String> = (0..sim::MAX_MATCH_PLAYERS + 1)
                .filter_map(|slot| player_line(slot, full, &catalogue))
                .collect();
            let colours: BTreeSet<&str> = lines
                .iter()
                .map(|line| line.split(" - ").nth(1).expect("colour"))
                .collect();
            assert_eq!((lines.len(), colours.len()), (10, 10), "{lines:?}");
            assert!(lines[9].starts_with("AI 9 - "), "{lines:?}");
        }
        // Unlock all is a separate, human-only choice.
        selection.unlock_all = true;
        assert_eq!(
            row_text(Row::UnlockAll, selection, &catalogue),
            "ALL UNLOCKED"
        );
        // With several enemies the human's defeat ends the match for the human at once.
        assert_eq!(end_text(None, true), Some("DEFEAT"));
        assert_eq!(end_text(None, false), None);
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
        // M7: the options screen opens from the menu, whatever the selection.
        assert_eq!(
            menu_effect(MenuButton::Options, start, COUNTS),
            MenuEffect::Options
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
            ..MenuSelection::default()
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
        assert_eq!(end_text(None, false), None);
        assert_eq!(
            end_text(
                Some(MatchOutcome::Won {
                    winner: HUMAN,
                    tick: 5
                }),
                false
            ),
            Some("VICTORY")
        );
        assert_eq!(
            end_text(
                Some(MatchOutcome::Won {
                    winner: OPPONENT,
                    tick: 5
                }),
                false
            ),
            Some("DEFEAT")
        );
        assert_eq!(
            end_text(Some(MatchOutcome::Draw { tick: 5 }), false),
            Some("DRAW")
        );
        let last = MenuSelection {
            map: 1,
            faction: 0,
            opponent: 1,
            difficulty: 2,
            ..MenuSelection::default()
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
        let view = catalogue.maps[0].map.view;
        assert_eq!(
            row_text(Row::Map, selection, &catalogue),
            format!(
                "{}  {} x {}",
                catalogue.maps[0].id.replace('_', " "),
                view.width,
                view.height
            )
        );
        assert_eq!(
            row_text(Row::Difficulty, selection, &catalogue),
            catalogue.ai.difficulties[selection.difficulty]
                .id
                .replace('_', " ")
        );
        let outside = MenuSelection {
            super_me: false,
            color: 0,
            map: catalogue.maps.len(),
            ..selection
        };
        assert_eq!(row_text(Row::Map, outside, &catalogue), "-");
    }
}
