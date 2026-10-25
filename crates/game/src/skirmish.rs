//! Game flow (PLAN.md section 7, M6): the catalogue of maps, factions, and difficulties loaded at
//! startup, the main menu with the choices of a match, the start of a match against the computer
//! players, the end screen with the winner, and restart (docs/DECISIONS.md D44).
//!
//! When the game starts, [`load_catalogue`] loads and checks everything a match can use: the unit,
//! economy, and combat files with every sprite (`crate::units::load_match_data`), the skirmish file
//! (factions, maps, start positions, starting credits) and the AI file (`sim::load_skirmish`,
//! `sim::load_ai`), and every map of the skirmish file with its tileset image
//! (`crate::map::check_map`). The files are then checked against each other ([`check_catalogue`]):
//! the start positions against their maps, the AI file against the factions, the starting credits
//! against the lobby of the rules file, and every match the menu can choose is set up once (M6
//! review 1 P1-1 and P2-1). The renderer receives only the
//! checked bytes, which are registered once (D16); nothing is read again when a match starts or
//! restarts. Any error ends the game with exit code 1, as before M6.
//!
//! The menu is the lobby of Red Alert 2 (RA1, docs/RA2_PARITY_PLAN.md section 3.5,
//! docs/DECISIONS.md R17): two columns of rows, each a choice with a `<` and a `>` button, and
//! the PLAYERS panel ([`menu_effect`], pure). The rows of the match: the map, the number of
//! enemy players, the team mode (computer players allied against the human, or free for all),
//! MAP GROWTH, the game speed, the team colour, the starting credits and the unit count (in the
//! ranges of the rules file). The rows of its rules: MCV start, short game, superweapons, crates
//! and MCV repacks, which are on as in Red Alert 2, and this game's extensions, which are off:
//! Super Me, its unit speed, and the tech tree with everything unlocked. The PLAYERS panel has a
//! line for every player of the match with its colour, its country (one of the nine with its
//! side, or RANDOM, which the simulation draws when the match is set up) and, for a computer
//! player, its difficulty. [`MenuSelection::players`] and [`MenuSelection::options`] turn the
//! rows into the players and options the simulation is set up from, and [`match_plan`] is the
//! one place that puts them together with the map.
//!
//! Starting a match ([`begin_match`]) ends the current one if any ([`end_match`]: the map, the
//! unit and building views, and every resource of the match are removed or reset), sets up a new
//! simulation with `sim::setup_match_configured` through `crate::sim_host::install_match` (human
//! player 0 at the map's first start position, computer player k at start position k), spawns
//! the map, sizes the minimap, and centres the camera on the human player's start. The countries
//! the players really play are then read back from the simulation ([`MatchPlayers`]) for the
//! texts that name a player: a notice says which country the human player plays, the notices
//! and the hover label call another player "Blue Player (France)", and a list of the players
//! with their countries is shown while the match is paused or over.
//!
//! With MAP GROWTH off (the default, docs/DECISIONS.md R4) a map is played as its file defines
//! it, by 2 players to as many as the file lists start positions. With it on every map can be
//! played by 2 to `sim::MAX_MATCH_PLAYERS` players, and the map of a match grows with them
//! ([`match_map`], `sim::enlarged_map`): two players play on the map file as it is, every
//! further player adds an eighth to its width and height, and the start positions the file
//! lacks are made on the enlarged map. The enlargement depends only on the map and the number of
//! players, so a restart plays on the same map.
//!
//! The seed of the simulation is taken from the clock when the match
//! starts and logged (M0 debt item 3). When the simulation reports the match's outcome, ticks stop
//! and the end screen shows victory, defeat, or a draw, with what decided it by the short or the
//! long game ([`end_reason`]), and Restart (the same choices, a new seed) and Main menu
//! ([`end_effect`], pure). Since M7 the menu also opens the options screen (`crate::options`,
//! docs/DECISIONS.md D52).

use std::collections::BTreeMap;
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
pub fn load(root: &Path, checked: &CheckedAssets) -> Result<Loaded, String> {
    let loaded = crate::units::load_match_data(root).map_err(|error| error.to_string())?;
    catalogue_of(root, &root.join(crate::units::DATA_DIR), loaded, checked)
}

/// What [`load`] returns: the catalogue, and the checked sprites of its unit and building types.
pub type Loaded = (
    Catalogue,
    Vec<crate::units::CheckedSheet>,
    Vec<crate::buildings::CheckedBuildingSprite>,
);

/// The catalogue of the definitions `loaded` from the data directory `data`: with the skirmish
/// and AI files of that directory and the maps of the asset directory `root`, checked
/// ([`check_catalogue`]).
fn catalogue_of(
    root: &Path,
    data: &Path,
    (loaded, buildings): (
        crate::units::LoadedUnits,
        Vec<crate::buildings::CheckedBuildingSprite>,
    ),
    checked: &CheckedAssets,
) -> Result<Loaded, String> {
    let definitions = loaded.definitions;
    let skirmish_file = crate::units::data_file(data, SKIRMISH_PATH);
    let ai_file = crate::units::data_file(data, AI_PATH);
    let skirmish =
        sim::load_skirmish(&skirmish_file, &definitions).map_err(|error| error.to_string())?;
    let ai = sim::load_ai(&ai_file, &definitions).map_err(|error| error.to_string())?;
    let mut maps = Vec::with_capacity(skirmish.maps.len());
    for def in &skirmish.maps {
        let asset_path = map_asset_path(&def.id);
        let map = check_map(root, &asset_path, checked).map_err(|error| {
            format!(
                "data file {}: map {:?}: {error}",
                skirmish_file.display(),
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
    check_catalogue(&catalogue, data)?;
    Ok((catalogue, loaded.sheets, buildings))
}

/// Checks the files of the catalogue against each other, so that every match the menu offers can
/// start and its computer player can play (M6 review 1 P1-1 and P2-1):
///
/// * every start position of every map is a cell of the map with passable terrain
///   (`sim::check_map_starts`);
/// * the AI file can be played by every faction: the producers of the buildings it builds are
///   starting buildings or are built before (`sim::check_ai_factions`);
/// * the starting credits of the skirmish file, which a match has while the menu's credits row
///   is left alone, are an amount that row offers ([`Lobby::offers_credits`]: within the range
///   of the rules file and on its step; RA1);
/// * every match the menu can choose can be set up: every map with MAP GROWTH on, enlarged for 2
///   to `sim::MAX_MATCH_PLAYERS` players ([`match_map`]), and as its file defines it with a
///   player on every start position the file lists (MAP GROWTH off, docs/DECISIONS.md R4), each
///   in a simulation of its own that is dropped afterwards. The fewest and the most players are
///   set up with every pairing of the sides, the player counts between them with one pairing
///   each, which changes with the count, so that the game still starts quickly (the difficulty
///   does not change the setup). The countries the trial setups played are logged.
///
/// Each error names the file and the field, or the map and the factions, and the reason; `data`
/// is the directory of the data files the catalogue was loaded from.
pub fn check_catalogue(catalogue: &Catalogue, data: &Path) -> Result<(), String> {
    let skirmish_path = &crate::units::data_file(data, SKIRMISH_PATH);
    let ai_path = &crate::units::data_file(data, AI_PATH);
    for map in 0..catalogue.maps.len() {
        sim::check_map_starts(
            skirmish_path,
            &catalogue.skirmish,
            map,
            &catalogue.maps[map].map.grid,
        )
        .map_err(|error| error.to_string())?;
    }
    sim::check_ai_factions(
        ai_path,
        &catalogue.ai,
        &catalogue.skirmish,
        &catalogue.definitions,
    )
    .map_err(|error| error.to_string())?;
    let lobby = Lobby::of(catalogue);
    if !lobby.offers_credits(catalogue.skirmish.starting_credits) {
        return Err(format!(
            "data file {}: starting_credits: {} is not an amount the rules file's lobby offers \
             ({} to {} in steps of {})",
            skirmish_path.display(),
            catalogue.skirmish.starting_credits,
            lobby.min_credits,
            lobby.max_credits,
            lobby.credits_step,
        ));
    }
    let factions = &catalogue.skirmish.factions;
    let name = |faction: usize| {
        factions
            .get(faction)
            .map_or("?", |faction| faction.id.as_str())
    };
    // How many trial setups each faction was a player of.
    let mut played: BTreeMap<usize, usize> = BTreeMap::new();
    let mut trials = 0_usize;
    for (map, entry) in catalogue.maps.iter().enumerate() {
        let cannot = |selection: MenuSelection, error: String| {
            format!(
                "data file {}: map {:?}: the match of faction {:?} against faction {:?} \
                 with {} computer players cannot be set up: {error}",
                skirmish_path.display(),
                entry.id,
                name(selection.faction()),
                name(selection.opponent()),
                selection.enemy_count(),
            )
        };
        let choice = |enemies: usize, map_growth: bool, (faction, opponent): (usize, usize)| {
            MenuSelection {
                map,
                enemies,
                map_growth,
                ..MenuSelection::default()
            }
            .with_countries(faction, opponent, factions.len())
        };
        let mut trial = |selection: MenuSelection, on: &MatchMap| -> Result<(), String> {
            crate::sim_host::check_match(&plan_on(catalogue, selection, 0, on))
                .map_err(|error| cannot(selection, error))?;
            trials += 1;
            for setup in selection.players() {
                *played.entry(setup.faction).or_insert(0) += 1;
            }
            Ok(())
        };
        // One pairing that changes with the number of enemies.
        let count = factions.len().max(1);
        let one = |enemies: usize| (enemies % count, (enemies / count) % count);
        let most = sim::MAX_MATCH_PLAYERS - sim::MIN_MATCH_PLAYERS;
        for enemies in 0..=most {
            // The map for this many players is the same for every pairing: enlarged once.
            let grown = match_map(catalogue, map, enemies + 2, true)
                .map_err(|error| cannot(choice(enemies, true, (0, 0)), error))?;
            // Every pairing of the sides with one opponent and with the most, each side played
            // by its countries in turn (docs/DECISIONS.md R5: every pairing of the nine countries
            // would be 81 trial setups per map and player count instead of 4); one in between.
            let groups = catalogue.skirmish.pairing_groups();
            let pairings: Vec<(usize, usize)> = if enemies == 0 || enemies == most {
                (0..groups.len())
                    .flat_map(|first| (0..groups.len()).map(move |second| (first, second)))
                    .enumerate()
                    .filter_map(|(pairing, (first, second))| {
                        Some((
                            SkirmishDefinitions::rotating(&groups[first], map + pairing)?,
                            SkirmishDefinitions::rotating(&groups[second], map + pairing + 1)?,
                        ))
                    })
                    .collect()
            } else {
                vec![one(enemies)]
            };
            for pairing in pairings {
                trial(choice(enemies, true, pairing), &grown)?;
            }
        }
        // MAP GROWTH off: the map as its file defines it, with a player on every start position
        // the file lists (two players on it are the first trial above).
        let filed = catalogue
            .skirmish
            .maps
            .get(map)
            .map_or(0, |def| def.starts.len().min(sim::MAX_MATCH_PLAYERS));
        if filed > sim::MIN_MATCH_PLAYERS {
            let selection = choice(filed - 2, false, one(map + filed));
            let as_filed = match_map(catalogue, map, filed, false)
                .map_err(|error| cannot(selection, error))?;
            trial(selection, &as_filed)?;
        }
    }
    info!(
        "catalogue check: {trials} trial setups on {} maps; players per country: {}",
        catalogue.maps.len(),
        played
            .iter()
            .map(|(&faction, count)| format!("{} {count}", name(faction)))
            .collect::<Vec<_>>()
            .join(", ")
    );
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
    /// Whether it is the map file's own map, cell for cell (MAP GROWTH off, a match of two
    /// players, or a map file of the largest size).
    pub as_filed: bool,
}

/// The number of players a map is enlarged for: the match's with MAP GROWTH on (`grow`), and
/// with it off the two of a one-against-one match, for whom a map is played as its file
/// defines it (docs/DECISIONS.md R4).
fn scaled_for(players: usize, grow: bool) -> usize {
    if grow {
        players
    } else {
        sim::MIN_MATCH_PLAYERS
    }
}

/// The size in cells of map `map` of the catalogue in a match with `players` players: the map
/// file's, or with MAP GROWTH on (`grow`) the enlarged one (`sim::enlarged_size`). For the menu.
pub fn match_map_size(
    catalogue: &Catalogue,
    map: usize,
    players: usize,
    grow: bool,
) -> Option<(u32, u32)> {
    let view = catalogue.maps.get(map)?.map.view;
    Some(sim::enlarged_size(
        view.width,
        view.height,
        scaled_for(players, grow),
    ))
}

/// Map `map` of the catalogue for a match with `players` players. With MAP GROWTH off (`grow`
/// false, the default, docs/DECISIONS.md R4) it is the map file's own, with the start positions
/// the file lists, which limit the players. With it on (`sim::enlarged_map`): the map file's own
/// for two, and one an eighth wider and higher for every further player, with the file's start
/// positions moved along and the missing ones made. A pure function of the map, the player
/// count and the option, so every match with these choices, and every restart, has the same map.
pub fn match_map(
    catalogue: &Catalogue,
    map: usize,
    players: usize,
    grow: bool,
) -> Result<MatchMap, String> {
    let (Some(entry), Some(def)) = (catalogue.maps.get(map), catalogue.skirmish.maps.get(map))
    else {
        return Err(format!("map {map} is not in the catalogue"));
    };
    let enlarged = sim::enlarged_map(
        &entry.map.grid,
        &entry.map.looks,
        &def.starts,
        scaled_for(players, grow),
    )
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
/// and options ([`MenuSelection::players`], [`MenuSelection::options`]) on the map of the match
/// ([`match_map`]), and the skirmish file with that map's start positions in place of the
/// file's, so that match setup and the computer players use the cells the bases really stand
/// on. Also returns the map, for drawing it.
pub fn match_plan(
    catalogue: &Catalogue,
    selection: MenuSelection,
    seed: u64,
) -> Result<(MatchPlan, MatchMap), String> {
    let map = match_map(
        catalogue,
        selection.map,
        selection.players().len(),
        selection.map_growth,
    )?;
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
        options: selection.options(),
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

/// What a match starts with, as `(units, buildings)`, when its players play the factions
/// `factions` (indices into the catalogue's), with the lobby's unit count `unit_count` and
/// "start with an MCV" as `mcv_start`: for a country its base unit or the Construction Yard it
/// deploys into, and the starting units of the unit count (`sim::starting_units`); for a faction
/// without a side its starting groups and buildings.
pub fn starting_totals(
    catalogue: &Catalogue,
    factions: impl IntoIterator<Item = usize>,
    unit_count: u32,
    mcv_start: bool,
) -> (usize, usize) {
    let definitions = &catalogue.definitions;
    factions
        .into_iter()
        .filter_map(|faction| catalogue.skirmish.factions.get(faction))
        .fold((0, 0), |(units, buildings), faction| match faction.side {
            // No type is drawn while every side lists one vehicle and one infantry type, so
            // the generator's seed does not matter.
            Some(side) => (
                units
                    + usize::from(mcv_start)
                    + sim::starting_units(
                        &catalogue.skirmish,
                        definitions,
                        side,
                        unit_count,
                        &mut sim::SimRng::new(0),
                    )
                    .len(),
                buildings + usize::from(!mcv_start),
            ),
            None => (
                units
                    + faction
                        .units
                        .iter()
                        .map(|group| group.count as usize)
                        .sum::<usize>(),
                buildings + faction.buildings.len(),
            ),
        })
}

/// [`starting_totals`] of the match of `selection`, with its unit count and MCV start; `None`
/// while one of its players has a RANDOM country, which only the simulation resolves
/// ([`MatchPlayers`] then says what is played).
#[cfg(test)]
pub fn selection_totals(catalogue: &Catalogue, selection: MenuSelection) -> Option<(usize, usize)> {
    let players = selection.players().len();
    let factions: Option<Vec<usize>> = selection.countries.iter().take(players).copied().collect();
    Some(starting_totals(
        catalogue,
        factions?,
        selection
            .unit_count
            .unwrap_or(Lobby::of(catalogue).unit_count),
        selection.mcv_start,
    ))
}

/// A player of the running match as the simulation set it up: with the country it really plays,
/// which for a RANDOM choice the simulation drew (`sim::MatchOptions::random_countries`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchPlayer {
    pub player: sim::PlayerId,
    /// The index of the player's faction in the catalogue's skirmish file.
    pub faction: usize,
    /// What the interface calls the country: `France (Allied)` ([`country_text`]).
    pub country: String,
    /// The country's name alone: `France`.
    pub label: String,
    pub side: Option<sim::Side>,
    /// The difficulty of a computer player, as the menu shows it.
    pub difficulty: Option<String>,
}

/// The players of the running match in the order of the menu's PLAYERS panel (the human player
/// first), for the texts that name a player by its country: the list shown while the match is
/// paused or over, the notices and the hover label. Empty without a match.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct MatchPlayers(pub Vec<MatchPlayer>);

impl MatchPlayers {
    /// The players `setups` as the simulation of `host` holds them: each with the faction whose
    /// country its state names (a faction without a side has none: then the faction of the
    /// setup).
    pub fn of(host: &SimHost, catalogue: &Catalogue, setups: &[PlayerSetup]) -> Self {
        let factions = &catalogue.skirmish.factions;
        Self(
            setups
                .iter()
                .map(|setup| {
                    let country = host
                        .players()
                        .and_then(|players| players.get(setup.player))
                        .and_then(sim::PlayerState::country);
                    let faction = country
                        .and_then(|country| {
                            factions
                                .iter()
                                .position(|faction| faction.country == Some(country))
                        })
                        .unwrap_or(setup.faction);
                    let def = factions.get(faction);
                    MatchPlayer {
                        player: setup.player,
                        faction,
                        country: country_text(Some(faction), catalogue),
                        label: def.map_or_else(|| "?".to_owned(), |def| def.label.clone()),
                        side: def.and_then(|def| def.side),
                        difficulty: match setup.controller {
                            Controller::Human => None,
                            Controller::Ai { difficulty } => {
                                Some(difficulty_text(difficulty, catalogue))
                            }
                        },
                    }
                })
                .collect(),
        )
    }

    pub fn get(&self, player: sim::PlayerId) -> Option<&MatchPlayer> {
        self.0.iter().find(|entry| entry.player == player)
    }

    /// The name of the country `player` plays, e.g. `France`.
    pub fn label(&self, player: sim::PlayerId) -> Option<&str> {
        self.get(player).map(|entry| entry.label.as_str())
    }

    /// The factions the players play, in their order.
    pub fn factions(&self) -> impl Iterator<Item = usize> + '_ {
        self.0.iter().map(|entry| entry.faction)
    }
}

/// How the interface names a player other than the human: by its team colour and, once the
/// match has started, the country it plays: `Blue Player (France)`.
pub fn player_name(colour: &str, country: Option<&str>) -> String {
    match country {
        Some(country) => format!("{colour} Player ({country})"),
        None => format!("{colour} Player"),
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
        // The art of the item the arsenal names (RA1): a new type shows that of its twin
        // until it has its own (docs/DECISIONS.md R11).
        let bytes = match crate::hd_art::checked(
            &root,
            "buildings",
            catalogue
                .definitions
                .art(sim::ProductionItem::Building(sprite.kind)),
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
        let bytes = match crate::hd_art::checked(
            &root,
            "units",
            catalogue
                .definitions
                .art(sim::ProductionItem::Unit(sheet.kind)),
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
        let art = catalogue.definitions.art(sim::ProductionItem::Unit(kind));
        if !root
            .join("sprites/hd/walk")
            .join(format!("{art}.png"))
            .is_file()
        {
            continue;
        }
        let size = Vec2::new(
            crate::hd_art::WALK_SIZE * 8.0,
            crate::hd_art::WALK_SIZE * crate::hd_art::WALK_FRAMES as f32,
        );
        let pair = crate::hd_art::checked(&root, "walk", art, size).and_then(|bytes| {
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

/// A row of the menu: a choice with a `<` and a `>` button. The rows of the match and of its
/// rules stand in two columns ([`Row::MATCH`], [`Row::RULES`]); the PLAYERS panel has a country
/// row for every player and a difficulty row for every computer player.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Row {
    Map,
    /// How many computer players: 1 to the start positions the map file lists less one, or with
    /// MAP GROWTH on 1 to `sim::MAX_MATCH_PLAYERS` minus one on every map.
    Enemies,
    /// ALLIED AI (the computer players fight the human and never each other) or FREE FOR ALL.
    TeamMode,
    /// Off: the map is played as its file defines it. On: it grows with the players, who may
    /// then be ten on every map (docs/DECISIONS.md R4).
    MapGrowth,
    /// How fast the match runs ([`crate::game_speed::SPEED_LEVELS`]); `[` and `]` and the
    /// sidebar change it during the match.
    GameSpeed,
    Color,
    /// The starting credits of every player, within the range and on the step of the rules file.
    Credits,
    /// The lobby's unit count, from which the starting units follow (`sim::starting_units`).
    UnitCount,
    /// On: every player starts with the base unit of its side. Off: with its Construction Yard.
    McvStart,
    /// On: a player without buildings and base unit is defeated. Off: without buildings and
    /// units.
    ShortGame,
    Superweapons,
    /// The option exists since RA1; crates appear once the simulation has them.
    Crates,
    BridgeDestruction,
    FogOfWar,
    OreGrowth,
    /// Whether a Construction Yard can be packed up into the base unit again.
    McvRepacks,
    SuperMe,
    /// How fast the human player's units move with Super Me ([`UNIT_SPEEDS`]).
    UnitSpeed,
    /// Standard tech tree, or every prerequisite waived for the human player.
    UnlockAll,
    /// The country of the player in this slot of the PLAYERS panel (0: the human player; k: the
    /// k-th computer player): one of the catalogue's, or RANDOM.
    Country(usize),
    /// The difficulty of the computer player in this slot (1 to 9).
    Difficulty(usize),
}

/// The unit speeds the Super Me player may choose, in percent of every unit type's speed.
pub const UNIT_SPEEDS: [u16; 5] = [100, 150, 200, 300, 400];

impl Row {
    /// The rows of the menu's first column: the match.
    pub const MATCH: [Row; 8] = [
        Row::Map,
        Row::Enemies,
        Row::TeamMode,
        Row::MapGrowth,
        Row::GameSpeed,
        Row::Color,
        Row::Credits,
        Row::UnitCount,
    ];

    /// The rows of the menu's second column: the lobby options of Red Alert 2, then this game's
    /// extensions.
    pub const RULES: [Row; 11] = [
        Row::McvStart,
        Row::ShortGame,
        Row::Superweapons,
        Row::Crates,
        Row::BridgeDestruction,
        Row::FogOfWar,
        Row::OreGrowth,
        Row::McvRepacks,
        Row::SuperMe,
        Row::UnitSpeed,
        Row::UnlockAll,
    ];

    /// How many rows the menu has: those of the two columns, a country row for each of the
    /// `sim::MAX_MATCH_PLAYERS` players a match may have, and a difficulty row for each of them
    /// but the human player.
    pub const COUNT: usize = Row::MATCH.len() + Row::RULES.len() + 2 * sim::MAX_MATCH_PLAYERS - 1;

    /// Every row of the menu: the two columns, then the rows of the PLAYERS panel slot by slot.
    #[cfg(test)]
    pub fn all() -> impl Iterator<Item = Row> {
        Row::MATCH
            .into_iter()
            .chain(Row::RULES)
            .chain((0..sim::MAX_MATCH_PLAYERS).flat_map(Row::of_slot))
    }

    /// The rows of slot `slot` of the PLAYERS panel: the country, and for a computer player the
    /// difficulty.
    pub fn of_slot(slot: usize) -> impl Iterator<Item = Row> {
        std::iter::once(Row::Country(slot)).chain((slot > 0).then_some(Row::Difficulty(slot)))
    }

    pub fn label(self) -> &'static str {
        match self {
            Row::Map => "Map",
            Row::Enemies => "Enemy players",
            Row::TeamMode => "Team mode",
            Row::MapGrowth => "Map growth",
            Row::GameSpeed => "Game speed",
            Row::Color => "Team color",
            Row::Credits => "Credits",
            Row::UnitCount => "Unit count",
            Row::McvStart => "MCV start",
            Row::ShortGame => "Short game",
            Row::Superweapons => "Superweapons",
            Row::Crates => "Crates",
            Row::BridgeDestruction => "Bridges",
            Row::FogOfWar => "FOG OF WAR",
            Row::OreGrowth => "Ore growth",
            Row::McvRepacks => "MCV repacks",
            Row::SuperMe => "Super Me",
            Row::UnitSpeed => "Unit speed",
            Row::UnlockAll => "Tech tree",
            Row::Country(_) => "Country",
            Row::Difficulty(_) => "Difficulty",
        }
    }
}

/// The choices of a match: indices into the catalogue's maps, factions, and difficulties, and
/// the options of the match. [`MenuSelection::players`] and [`MenuSelection::options`] turn
/// them into what the simulation is set up from.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuSelection {
    pub map: usize,
    /// Computer players minus one (0: the classic one-against-one).
    pub enemies: usize,
    pub color: usize,
    /// Index into [`crate::game_speed::SPEED_LEVELS`].
    pub game_speed: usize,
    /// Per slot of the PLAYERS panel (0: the human player; k: the k-th computer player,
    /// `PlayerId(k)`), the index of the player's country among the catalogue's factions, or
    /// `None` for RANDOM: the simulation draws it when the match is set up.
    pub countries: [Option<usize>; sim::MAX_MATCH_PLAYERS],
    /// Per slot, the index of the computer player's difficulty in the AI file (slot 0 is not
    /// read).
    pub difficulties: [usize; sim::MAX_MATCH_PLAYERS],
    /// The starting credits of every player; `None` for those of the skirmish file, which is
    /// what the row shows until it is changed (`sim::MatchOptions::starting_credits`).
    pub credits: Option<i64>,
    /// The lobby's unit count; `None` for the default of the rules file
    /// (`sim::MatchOptions::unit_count`).
    pub unit_count: Option<u32>,
    pub mcv_start: bool,
    pub short_game: bool,
    pub superweapons: bool,
    pub crates: bool,
    pub bridge_destruction: bool,
    pub fog_of_war: bool,
    pub ore_grows: bool,
    pub mcv_repacks: bool,
    /// Off: the computer players are allied against the human player.
    pub free_for_all: bool,
    /// Whether the map grows with the players (docs/DECISIONS.md R4); an option of the game,
    /// not of the simulation ([`match_map`]).
    pub map_growth: bool,
    pub super_me: bool,
    /// Every building prerequisite waived for the human player (match option, like Super Me).
    pub unlock_all: bool,
    /// Index into [`UNIT_SPEEDS`]; it takes effect with Super Me only.
    pub unit_speed: usize,
}

impl Default for MenuSelection {
    /// The lobby of Red Alert 2 as far as it is known without a catalogue
    /// (`sim::MatchOptions::default`), with the extensions of this game off: the first map, one
    /// computer player, the human player with the first country and every computer player with
    /// a RANDOM one, the first difficulty.
    fn default() -> Self {
        let lobby = sim::MatchOptions::default();
        let mut countries = [None; sim::MAX_MATCH_PLAYERS];
        countries[0] = Some(0);
        Self {
            map: 0,
            enemies: 0,
            color: 0,
            game_speed: crate::game_speed::NORMAL_SPEED,
            countries,
            difficulties: [0; sim::MAX_MATCH_PLAYERS],
            credits: lobby.starting_credits,
            unit_count: lobby.unit_count,
            mcv_start: lobby.mcv_start,
            short_game: lobby.short_game,
            superweapons: lobby.superweapons,
            crates: lobby.crates,
            bridge_destruction: lobby.bridge_destruction,
            fog_of_war: lobby.fog_of_war,
            ore_grows: lobby.ore_grows,
            mcv_repacks: lobby.mcv_repacks,
            free_for_all: lobby.free_for_all,
            map_growth: false,
            super_me: false,
            unlock_all: false,
            unit_speed: 0,
        }
    }
}

/// What the credits and unit count rows offer: the lobby settings of the rules file
/// (`[MultiplayerDialogSettings]`), with the starting credits of the skirmish file as the
/// amount a match has while the row is left alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lobby {
    pub min_credits: i64,
    pub max_credits: i64,
    /// At least 1.
    pub credits_step: i64,
    /// The starting credits of the skirmish file.
    pub credits: i64,
    pub min_unit_count: u32,
    pub max_unit_count: u32,
    /// The unit count of the rules file.
    pub unit_count: u32,
}

impl Lobby {
    /// The lobby of the catalogue's rules file. Without one (the legacy ruleset fixture) the
    /// rows offer one entry each: the credits of the skirmish file and a unit count of 0.
    pub fn of(catalogue: &Catalogue) -> Self {
        let credits = catalogue.skirmish.starting_credits;
        match catalogue.definitions.rules().map(|rules| rules.dialog) {
            Some(dialog) => Self {
                min_credits: i64::from(dialog.min_credits),
                max_credits: i64::from(dialog.max_credits),
                credits_step: i64::from(dialog.credits_step.max(1)),
                credits,
                min_unit_count: u32::try_from(dialog.min_unit_count).unwrap_or(0),
                max_unit_count: u32::try_from(dialog.max_unit_count).unwrap_or(0),
                unit_count: u32::try_from(dialog.unit_count).unwrap_or(0),
            },
            None => Self {
                min_credits: credits,
                max_credits: credits,
                credits_step: 1,
                credits,
                min_unit_count: 0,
                max_unit_count: 0,
                unit_count: 0,
            },
        }
    }

    /// Whether the credits row offers `credits`: within the range and on a step from its
    /// minimum, which is what the simulation accepts (`sim::setup_match_configured`).
    pub fn offers_credits(self, credits: i64) -> bool {
        (self.min_credits..=self.max_credits).contains(&credits)
            && (credits - self.min_credits) % self.credits_step.max(1) == 0
    }

    /// How many amounts the credits row has.
    pub fn credit_amounts(self) -> usize {
        let steps = (self.max_credits - self.min_credits).max(0) / self.credits_step.max(1);
        usize::try_from(steps).map_or(1, |steps| steps.saturating_add(1))
    }

    pub fn offers_unit_count(self, count: u32) -> bool {
        (self.min_unit_count..=self.max_unit_count).contains(&count)
    }

    /// How many entries the unit count row has.
    pub fn unit_counts(self) -> usize {
        self.max_unit_count.saturating_sub(self.min_unit_count) as usize + 1
    }
}

/// How many entries the rows have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuCounts {
    pub maps: usize,
    pub factions: usize,
    /// Whether a country row offers RANDOM: the skirmish file has countries to draw from.
    pub random: bool,
    pub difficulties: usize,
    /// Per map, the start positions its file lists.
    pub starts: [usize; sim::MAX_MAPS],
    pub lobby: Lobby,
}

impl MenuCounts {
    pub fn of(catalogue: &Catalogue) -> Self {
        let mut starts = [0; sim::MAX_MAPS];
        for (slot, def) in starts.iter_mut().zip(&catalogue.skirmish.maps) {
            *slot = def.starts.len();
        }
        Self {
            maps: catalogue.maps.len(),
            factions: catalogue.skirmish.factions.len(),
            random: !catalogue.skirmish.countries().is_empty(),
            difficulties: catalogue.ai.difficulties.len(),
            starts,
            lobby: Lobby::of(catalogue),
        }
    }

    /// The most computer players a match on map `map` may have: one per start position its file
    /// lists beyond the human player's, or with MAP GROWTH on (`growth`)
    /// `sim::MAX_MATCH_PLAYERS` minus one on every map of the catalogue, however many start
    /// positions its file lists (the map of a match is then enlarged for its players and gets
    /// the missing ones, [`match_map`]). 0 for a map outside the catalogue.
    pub fn enemies(self, map: usize, growth: bool) -> usize {
        let most = sim::MAX_MATCH_PLAYERS - 1;
        match self.starts.get(map) {
            Some(_) if map >= self.maps => 0,
            Some(_) if growth => most,
            Some(&starts) => starts.saturating_sub(1).min(most),
            None => 0,
        }
    }
}

impl MenuSelection {
    /// What the menu's rows show at first: the lobby of Red Alert 2 ([`MenuSelection::default`])
    /// on the first map, the human player with the first country (America, with the shipped
    /// data) and every computer player with a RANDOM one and the middle difficulty (the AI file
    /// lists them from the easiest). A skirmish file without countries has nothing to draw
    /// from: its computer players take the factions in turn, from the first.
    pub fn initial(catalogue: &Catalogue) -> Self {
        let selection = Self {
            difficulties: [catalogue.ai.difficulties.len() / 2; sim::MAX_MATCH_PLAYERS],
            ..Self::default()
        };
        if catalogue.skirmish.countries().is_empty() {
            selection.with_countries(0, 0, catalogue.skirmish.factions.len())
        } else {
            selection
        }
    }

    /// [`MenuSelection::initial`] with a fixed country for every player: the human player with
    /// the first, the computer players in turn from the first country of another side (with the
    /// shipped data America against Libya, then Iraq, Cuba, ...). For the review scenes and the
    /// tests that need to know what a match holds before it is set up.
    pub fn versus(catalogue: &Catalogue) -> Self {
        let factions = &catalogue.skirmish.factions;
        let other = factions
            .iter()
            .position(|faction| Some(faction.side) != factions.first().map(|own| own.side))
            .unwrap_or(0);
        Self::initial(catalogue).with_countries(0, other, factions.len())
    }

    /// The selection with the human player playing faction `human` and the computer players the
    /// factions in turn from `opponent` (the first plays it, the second the one after it, and
    /// so on through the `factions` of the catalogue), so that a match with several enemies
    /// fields several countries.
    pub fn with_countries(mut self, human: usize, opponent: usize, factions: usize) -> Self {
        for (slot, country) in self.countries.iter_mut().enumerate() {
            *country = Some(match slot {
                0 => human,
                slot => (opponent + slot - 1) % factions.max(1),
            });
        }
        self
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

    /// The faction of the human player's country, or the first while it is RANDOM.
    pub fn faction(self) -> usize {
        self.countries[0].unwrap_or(0)
    }

    /// The faction of the first computer player's country, or the first while it is RANDOM.
    pub fn opponent(self) -> usize {
        self.countries[1].unwrap_or(0)
    }

    /// The entry of `row` that is chosen and how many entries the row has, as `(index, count)`.
    /// The entries of a row are numbered from its lowest or first: OFF before ON, ALLIED AI
    /// before FREE FOR ALL, the countries in the catalogue's order and then RANDOM. A row that
    /// does not exist has no entry.
    pub fn position(self, row: Row, counts: MenuCounts) -> (usize, usize) {
        let switch = |on: bool| (usize::from(on), 2);
        let lobby = counts.lobby;
        match row {
            Row::Map => (self.map, counts.maps),
            Row::Enemies => (self.enemies, counts.enemies(self.map, self.map_growth)),
            Row::TeamMode => switch(self.free_for_all),
            Row::MapGrowth => switch(self.map_growth),
            Row::GameSpeed => (self.game_speed, crate::game_speed::SPEED_LEVELS.len()),
            Row::Color => (self.color, crate::presentation::TEAM_PALETTE.len()),
            Row::Credits => (
                usize::try_from(
                    (self.credits.unwrap_or(lobby.credits) - lobby.min_credits)
                        / lobby.credits_step.max(1),
                )
                .unwrap_or(usize::MAX),
                lobby.credit_amounts(),
            ),
            Row::UnitCount => (
                self.unit_count
                    .unwrap_or(lobby.unit_count)
                    .checked_sub(lobby.min_unit_count)
                    .map_or(usize::MAX, |above| above as usize),
                lobby.unit_counts(),
            ),
            Row::McvStart => switch(self.mcv_start),
            Row::ShortGame => switch(self.short_game),
            Row::Superweapons => switch(self.superweapons),
            Row::Crates => switch(self.crates),
            Row::BridgeDestruction => switch(self.bridge_destruction),
            Row::FogOfWar => switch(self.fog_of_war),
            Row::OreGrowth => switch(self.ore_grows),
            Row::McvRepacks => switch(self.mcv_repacks),
            Row::SuperMe => switch(self.super_me),
            Row::UnitSpeed => (self.unit_speed, UNIT_SPEEDS.len()),
            Row::UnlockAll => switch(self.unlock_all),
            Row::Country(slot) => match self.countries.get(slot) {
                Some(country) => (
                    match *country {
                        Some(faction) if faction < counts.factions => faction,
                        // No faction of the catalogue: no entry of the row.
                        Some(_) => usize::MAX,
                        None => counts.factions,
                    },
                    counts.factions + usize::from(counts.random),
                ),
                None => (0, 0),
            },
            Row::Difficulty(slot) => match self.difficulties.get(slot) {
                Some(&difficulty) if slot > 0 => (difficulty, counts.difficulties),
                _ => (0, 0),
            },
        }
    }

    /// Chooses entry `index` of `row` ([`MenuSelection::position`]).
    fn place(&mut self, row: Row, index: usize, counts: MenuCounts) {
        let lobby = counts.lobby;
        match row {
            Row::Map => self.map = index,
            Row::Enemies => self.enemies = index,
            Row::TeamMode => self.free_for_all = index != 0,
            Row::MapGrowth => self.map_growth = index != 0,
            Row::GameSpeed => self.game_speed = index,
            Row::Color => self.color = index,
            Row::Credits => {
                self.credits = Some(
                    lobby.min_credits
                        + lobby
                            .credits_step
                            .saturating_mul(i64::try_from(index).unwrap_or(0)),
                );
            }
            Row::UnitCount => {
                self.unit_count = Some(
                    lobby
                        .min_unit_count
                        .saturating_add(u32::try_from(index).unwrap_or(0)),
                );
            }
            Row::McvStart => self.mcv_start = index != 0,
            Row::ShortGame => self.short_game = index != 0,
            Row::Superweapons => self.superweapons = index != 0,
            Row::Crates => self.crates = index != 0,
            Row::BridgeDestruction => self.bridge_destruction = index != 0,
            Row::FogOfWar => self.fog_of_war = index != 0,
            Row::OreGrowth => self.ore_grows = index != 0,
            Row::McvRepacks => self.mcv_repacks = index != 0,
            Row::SuperMe => self.super_me = index != 0,
            Row::UnitSpeed => self.unit_speed = index,
            Row::UnlockAll => self.unlock_all = index != 0,
            Row::Country(slot) => {
                if let Some(country) = self.countries.get_mut(slot) {
                    *country = (index < counts.factions).then_some(index);
                }
            }
            Row::Difficulty(slot) => {
                if let Some(difficulty) = self.difficulties.get_mut(slot) {
                    *difficulty = index;
                }
            }
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

    /// Whether every choice names an entry of the catalogue and of the lobby: every row of the
    /// two columns, and the rows of the players the match has.
    pub fn valid(self, counts: MenuCounts) -> bool {
        let chosen = |row: Row| {
            let (index, count) = self.position(row, counts);
            index < count
        };
        Row::MATCH.into_iter().chain(Row::RULES).all(chosen)
            && (0..self.players().len()).flat_map(Row::of_slot).all(chosen)
            && self
                .credits
                .is_none_or(|credits| counts.lobby.offers_credits(credits))
            && self
                .unit_count
                .is_none_or(|count| counts.lobby.offers_unit_count(count))
    }

    /// How many computer players the match has.
    pub fn enemy_count(self) -> usize {
        self.enemies + 1
    }

    /// The players of the match: the human player at the first start position of the match's
    /// map ([`match_map`]), computer player k (`PlayerId(k)`, from 1) at start position k with
    /// the difficulty of its slot. Each computer player is independent: its own id, base,
    /// credits, queues, AI state and colour. A player plays the country of its slot; the faction
    /// of one with a RANDOM country is not read ([`MenuSelection::options`] lists it for the
    /// simulation to draw).
    pub fn players(self) -> Vec<PlayerSetup> {
        (0..=self.enemy_count())
            .map(|slot| PlayerSetup {
                player: sim::PlayerId(u8::try_from(slot).unwrap_or(u8::MAX)),
                faction: self.countries.get(slot).copied().flatten().unwrap_or(0),
                start: slot,
                controller: if slot == 0 {
                    Controller::Human
                } else {
                    Controller::Ai {
                        difficulty: self.difficulties.get(slot).copied().unwrap_or(0),
                    }
                },
            })
            .collect()
    }

    /// The options of the match (`sim::MatchOptions`): the rows of the lobby as they stand, the
    /// players whose country is RANDOM, and Super Me, its unit speed and "unlock all" for the
    /// human player alone. MAP GROWTH is not among them: the game enlarges the map before the
    /// setup, or does not ([`match_map`]).
    pub fn options(self) -> sim::MatchOptions {
        let human = |on: bool| if on { vec![HUMAN] } else { Vec::new() };
        sim::MatchOptions {
            super_players: human(self.super_me),
            unlock_players: human(self.unlock_all),
            unit_speed_percent: self.unit_speed_percent(),
            free_for_all: self.free_for_all,
            random_countries: self
                .players()
                .iter()
                .zip(self.countries)
                .filter(|(_, country)| country.is_none())
                .map(|(setup, _)| setup.player)
                .collect(),
            starting_credits: self.credits,
            unit_count: self.unit_count,
            mcv_start: self.mcv_start,
            short_game: self.short_game,
            superweapons: self.superweapons,
            crates: self.crates,
            mcv_repacks: self.mcv_repacks,
            // The lobby has no row for it yet: ore grows, as in the reference's default.
            ore_grows: self.ore_grows,
            bridge_destruction: self.bridge_destruction,
            fog_of_war: self.fog_of_war,
        }
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
        let (current, count) = selection.position(row, counts);
        if count == 0 {
            return MenuEffect::Nothing;
        }
        let current = current.min(count - 1);
        let mut next = selection;
        next.place(
            row,
            if forward {
                (current + 1) % count
            } else {
                (current + count - 1) % count
            },
            counts,
        );
        // A map that admits fewer enemies keeps as many as it can hold, as does a map that no
        // longer grows.
        next.enemies = next
            .enemies
            .min(counts.enemies(next.map, next.map_growth).saturating_sub(1));
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

/// How the menu names faction `faction` of the catalogue: a country with its side,
/// `France (Allied)`; a faction without a side by its label; `RANDOM` for `None`.
pub fn country_text(faction: Option<usize>, catalogue: &Catalogue) -> String {
    let Some(faction) = faction else {
        return "RANDOM".to_owned();
    };
    match catalogue.skirmish.factions.get(faction) {
        Some(def) => match def.side {
            Some(side) => format!("{} ({})", def.label, crate::hover_info::title(side.id())),
            None => def.label.clone(),
        },
        None => "-".to_owned(),
    }
}

/// How the menu names difficulty `difficulty` of the AI file: its id in words with capitals,
/// `Medium`.
pub fn difficulty_text(difficulty: usize, catalogue: &Catalogue) -> String {
    catalogue
        .ai
        .difficulties
        .get(difficulty)
        .map_or_else(|| "-".to_owned(), |def| crate::hover_info::title(&def.id))
}

/// The text of a menu row. The map row names the map (its id with underscores as spaces) and
/// the size in cells it has for the chosen players, `liberty bay  256 x 256`, which with MAP
/// GROWTH on changes with the enemy players row ([`match_map_size`]).
pub fn row_text(row: Row, selection: MenuSelection, catalogue: &Catalogue) -> String {
    let switch = |on: bool| if on { "ON" } else { "OFF" }.to_owned();
    let counts = MenuCounts::of(catalogue);
    match row {
        Row::Map => {
            let size = match_map_size(
                catalogue,
                selection.map,
                selection.players().len(),
                selection.map_growth,
            );
            match (catalogue.maps.get(selection.map), size) {
                (Some(map), Some((width, height))) => {
                    format!("{}  {width} x {height}", map.id.replace('_', " "))
                }
                _ => "-".to_owned(),
            }
        }
        Row::Enemies => format!(
            "{} of {}",
            selection.enemy_count(),
            counts.enemies(selection.map, selection.map_growth)
        ),
        Row::TeamMode => if selection.free_for_all {
            "FREE FOR ALL"
        } else {
            "ALLIED AI"
        }
        .to_owned(),
        Row::MapGrowth => switch(selection.map_growth),
        Row::GameSpeed => crate::game_speed::SPEED_LEVELS
            .get(selection.game_speed)
            .map_or_else(|| "-".to_owned(), |&f| crate::game_speed::speed_label(f)),
        Row::Color => crate::presentation::TEAM_PALETTE
            .get(selection.color)
            .map_or_else(|| "-".to_owned(), |colour| colour.0.to_owned()),
        Row::Credits => selection
            .credits
            .unwrap_or(counts.lobby.credits)
            .to_string(),
        Row::UnitCount => selection
            .unit_count
            .unwrap_or(counts.lobby.unit_count)
            .to_string(),
        Row::McvStart => switch(selection.mcv_start),
        Row::ShortGame => switch(selection.short_game),
        Row::Superweapons => switch(selection.superweapons),
        Row::Crates => switch(selection.crates),
        Row::FogOfWar => switch(selection.fog_of_war),
        Row::OreGrowth => switch(selection.ore_grows),
        Row::BridgeDestruction => if selection.bridge_destruction {
            "DESTRUCTIBLE"
        } else {
            "INDESTRUCTIBLE"
        }
        .to_owned(),
        Row::McvRepacks => switch(selection.mcv_repacks),
        Row::SuperMe => if selection.super_me {
            "ON / 10x / $1"
        } else {
            "OFF"
        }
        .to_owned(),
        Row::UnitSpeed => {
            let Some(&percent) = UNIT_SPEEDS.get(selection.unit_speed) else {
                return "-".to_owned();
            };
            let label = crate::game_speed::speed_label(f32::from(percent) / 100.0);
            if selection.super_me {
                label
            } else {
                format!("{label} - SUPER ME OFF")
            }
        }
        Row::UnlockAll => if selection.unlock_all {
            "ALL UNLOCKED"
        } else {
            "STANDARD"
        }
        .to_owned(),
        Row::Country(slot) => match selection.countries.get(slot) {
            Some(&country) => country_text(country, catalogue),
            None => "-".to_owned(),
        },
        Row::Difficulty(slot) => match selection.difficulties.get(slot) {
            Some(&difficulty) if slot > 0 => difficulty_text(difficulty, catalogue),
            _ => "-".to_owned(),
        },
    }
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
// The choices of a match are a few hundred bytes since they hold a country and a difficulty for
// each of ten players; an effect is made once per click and they stay `Copy`.
#[allow(clippy::large_enum_variant)]
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

/// The line under the headline of the end screen ([`end_text`]), which says what decided the
/// match by its rule (`sim::MatchRules::short_game`, docs/DECISIONS.md R13): in a short game a
/// player is defeated once it has neither a building nor an MCV, in a long game once it has
/// neither a building nor a unit. `None` while the match goes on.
pub fn end_reason(
    outcome: Option<MatchOutcome>,
    human_defeated: bool,
    short_game: bool,
) -> Option<&'static str> {
    Some(match (end_text(outcome, human_defeated)?, short_game) {
        ("VICTORY", true) => "Short game: no enemy has a building or an MCV left",
        ("VICTORY", false) => "Long game: no enemy has a building or a unit left",
        ("DEFEAT", true) => "Short game: you have no building and no MCV left",
        ("DEFEAT", false) => "Long game: you have no building and no unit left",
        (_, true) => "Short game: the last players lost their buildings and MCVs together",
        (_, false) => "Long game: the last players lost their buildings and units together",
    })
}

/// The line that tells the human player which country it plays when a match starts, which is
/// news when the choice was RANDOM: `You play France (Allied)`.
pub fn start_text(country: &str) -> String {
    format!("You play {country}")
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
/// ([`end_match`]), sets up the simulation on the map of the match ([`match_plan`]), spawns that
/// map, sizes the minimap and the camera's bounds for it, centres the camera on the human
/// player's start, and records the countries the players really play ([`MatchPlayers`]: the
/// simulation has drawn the RANDOM ones), of which the human player's is announced. A setup
/// error is logged and ends the game with exit code 1, as before M6; nothing of a new match is
/// then shown.
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
    // The countries as the match has them: the simulation has drawn the RANDOM ones.
    let played = MatchPlayers::of(world.resource::<SimHost>(), &catalogue, &plan.players);
    let options = &plan.options;
    info!(
        "match on map {} ({} x {} cells{}, seed {seed}): {}; {} credits, unit count {}, {}, {} \
         game, superweapons {}, crates {}, MCV repacks {}, {}{}{}: {} buildings and {} units \
         spawned",
        entry.id,
        map.map.view.width,
        map.map.view.height,
        if selection.map_growth {
            ", map growth"
        } else {
            ""
        },
        played
            .0
            .iter()
            .map(|entry| match &entry.difficulty {
                Some(difficulty) => format!("{} ({difficulty})", entry.country),
                None => entry.country.clone(),
            })
            .collect::<Vec<_>>()
            .join(" against "),
        row_text(Row::Credits, selection, &catalogue),
        row_text(Row::UnitCount, selection, &catalogue),
        if options.mcv_start {
            "MCV start"
        } else {
            "yard start"
        },
        if options.short_game { "short" } else { "long" },
        row_text(Row::Superweapons, selection, &catalogue),
        row_text(Row::Crates, selection, &catalogue),
        row_text(Row::McvRepacks, selection, &catalogue),
        if options.free_for_all {
            "free for all"
        } else {
            "allied computer players"
        },
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
    if let Some(own) = played.get(HUMAN) {
        let [r, g, b] = crate::presentation::TEAM_PALETTE
            [crate::presentation::team_color_index(selection.color, HUMAN)]
        .1;
        crate::notices::announce(world, start_text(&own.country), Color::srgb_u8(r, g, b));
    }
    world.insert_resource(played);
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
    world.insert_resource(MatchPlayers::default());
    crate::minimap::size_minimap(world, 1, 1);
    *world.resource_mut::<Screen>() = Screen::Menu;
}

// --- The screens ----------------------------------------------------------------------------

/// The menu's full-window panel.
#[derive(Component, Debug)]
pub struct MenuRoot;

/// A block of the menu: its heading lines, its columns and its row of buttons. Together with
/// the buttons and the texts they bound what the menu covers ([`menu_size`]).
#[derive(Component, Debug)]
pub struct MenuBlock;

/// The end screen's panel.
#[derive(Component, Debug)]
pub struct EndRoot;

/// The list of the match's players with the countries they play, shown over the map while the
/// match is paused or over.
#[derive(Component, Debug)]
pub struct PlayedRoot;

/// A text of the menu or the end screen that changes.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenText {
    /// The chosen entry of a menu row.
    Row(Row),
    /// The end screen's headline.
    Outcome,
    /// The end screen's line under the headline: what decided the match ([`end_reason`]).
    Reason,
    /// The name of a line of the menu's player list: player `n` of the match (0 the human
    /// player).
    Player(usize),
    /// A line of the list of the running match's players ([`PlayedRoot`]).
    Played(usize),
}

/// A line of the menu's player list, shown for the players of the chosen match.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerLine(pub usize);

/// The colour swatch of a line of the menu's player list.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerSwatch(pub usize);

/// A line of the list of the running match's players.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayedLine(pub usize);

/// The colour swatch of a line of the list of the running match's players.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayedSwatch(pub usize);

/// The name of slot `slot` of the menu's player list, `You - Blue` or `AI 2 - Gold` (ASCII: the
/// interface font has no middle dot): who the player is and the name of its team colour, which
/// the notices and the hover label call it by during the match. `None` beyond the match's
/// players.
pub fn player_tag(slot: usize, selection: MenuSelection) -> Option<String> {
    let setup = selection.players().into_iter().nth(slot)?;
    let colour = crate::presentation::TEAM_PALETTE
        [crate::presentation::team_color_index(selection.color, setup.player)]
    .0;
    Some(match setup.controller {
        Controller::Human => format!("You - {colour}"),
        Controller::Ai { .. } => format!("AI {slot} - {colour}"),
    })
}

/// Line `slot` of the list of the running match's players: its name as in the menu
/// ([`player_tag`]), the country it really plays (never RANDOM), a computer player's difficulty,
/// and `defeated` once it is: `You - Blue - France (Allied)` or `AI 2 - Gold - Iraq (Soviet),
/// Medium, defeated`. `None` beyond the match's players.
pub fn played_line(
    slot: usize,
    played: &MatchPlayers,
    colour_choice: usize,
    defeated: bool,
) -> Option<String> {
    let entry = played.0.get(slot)?;
    let colour = crate::presentation::TEAM_PALETTE
        [crate::presentation::team_color_index(colour_choice, entry.player)]
    .0;
    let mut line = match &entry.difficulty {
        None => format!("You - {colour} - {}", entry.country),
        Some(difficulty) => format!("AI {slot} - {colour} - {}, {difficulty}", entry.country),
    };
    if defeated {
        line.push_str(", defeated");
    }
    Some(line)
}

/// Shows the lines and swatches of the two player lists: the menu's for the current choices,
/// and the running match's.
#[allow(clippy::type_complexity)]
fn update_player_roster(
    selection: Res<MenuSelection>,
    played: Option<Res<MatchPlayers>>,
    teams: Option<Res<crate::presentation::TeamColors>>,
    mut lines: Query<
        (Option<&PlayerLine>, Option<&PlayedLine>, &mut Node),
        Or<(With<PlayerLine>, With<PlayedLine>)>,
    >,
    mut swatches: Query<
        (
            Option<&PlayerSwatch>,
            Option<&PlayedSwatch>,
            &mut BackgroundColor,
        ),
        Or<(With<PlayerSwatch>, With<PlayedSwatch>)>,
    >,
) {
    let chosen = selection.players().len();
    let playing = played.as_ref().map_or(0, |played| played.0.len());
    for (menu, played, mut node) in &mut lines {
        let shown = match (menu, played) {
            (Some(line), _) => line.0 < chosen,
            (None, Some(line)) => line.0 < playing,
            (None, None) => continue,
        };
        let display = if shown { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    }
    // The colours of the running match are those it was started with.
    let playing_choice = teams.map_or(selection.color, |teams| teams.0);
    for (menu, played, mut colour) in &mut swatches {
        let (choice, slot) = match (menu, played) {
            (Some(swatch), _) => (selection.color, swatch.0),
            (None, Some(swatch)) => (playing_choice, swatch.0),
            (None, None) => continue,
        };
        let rgb = crate::presentation::TEAM_PALETTE[crate::presentation::team_color_index(
            choice,
            sim::PlayerId(u8::try_from(slot).unwrap_or(0)),
        )]
        .1;
        let wanted = Color::srgb_u8(rgb[0], rgb[1], rgb[2]);
        if colour.0 != wanted {
            colour.0 = wanted;
        }
    }
}

/// A text of the menu and the screens: on one line, never wrapped, so that a text too long for
/// its place shows as too wide (which the smoke test checks) instead of pushing its row apart.
fn screen_text(value: impl Into<String>, size: f32) -> (Text, TextFont, TextColor, TextLayout) {
    (
        Text::new(value),
        TextFont::from_font_size(size),
        TextColor(Color::srgb(0.92, 0.94, 0.97)),
        TextLayout::no_wrap(),
    )
}

fn screen_button(width: f32, height: f32) -> impl Bundle {
    (
        Button,
        Node {
            width: Val::Px(width),
            height: Val::Px(height),
            flex_shrink: 0.0,
            border: UiRect::all(Val::Px(1.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BorderColor::all(Color::srgb(0.43, 0.46, 0.32)),
        BackgroundColor(Color::srgb(0.16, 0.21, 0.20)),
    )
}

/// A node of a fixed size that centres what it holds: the place of one text.
fn cell(width: f32, height: f32, justify: JustifyContent) -> Node {
    Node {
        width: Val::Px(width),
        height: Val::Px(height),
        flex_shrink: 0.0,
        justify_content: justify,
        align_items: AlignItems::Center,
        ..default()
    }
}

// The measures of the menu, in logical pixels. Every piece has a fixed size, so the menu covers
// the same ground in every window that holds it ([`menu_size`]).

/// The size the menu must fit in: the smallest window the game supports.
pub const MENU_FIT: Vec2 = Vec2::new(1280.0, 720.0);

/// Gap between the blocks of the menu, one below the other.
const BLOCK_GAP: f32 = 10.0;
/// Gap between the columns of the menu.
const COLUMN_GAP: f32 = 16.0;
/// Heights of the heading lines: the two small ones, the title, the rule under them, the row of
/// buildings and the prompt.
const HEADING_HEIGHTS: [f32; 6] = [16.0, 54.0, 16.0, 2.0, 68.0, 14.0];
/// Height of a column's title line.
const TITLE_HEIGHT: f32 = 16.0;
/// A row of the two columns: its height, the gap below it, its font, and the gap between its
/// label, buttons and value.
const ROW_HEIGHT: f32 = 26.0;
const ROW_GAP: f32 = 4.0;
const ROW_FONT: f32 = 14.0;
const CELL_GAP: f32 = 5.0;
/// Width of a row's `<` and `>` buttons.
const ARROW_WIDTH: f32 = 24.0;
/// Widths of the label and the value of a row of the first column: wide enough for
/// `Enemy players` and for the longest map with its size, `continental divide  256 x 256`.
const MATCH_WIDTHS: (f32, f32) = (114.0, 250.0);
/// The same of the second column: `Superweapons` and `1.5x - SUPER ME OFF`.
const RULES_WIDTHS: (f32, f32) = (104.0, 164.0);
/// A line of the player list: its height, font, and the widths of its swatch, its name
/// (`AI 9 - Orange`), its `<` and `>` buttons, its country (`Great Britain (Allied)`) and its
/// difficulty (`Medium`).
const PLAYER_HEIGHT: f32 = 22.0;
const PLAYER_FONT: f32 = 12.0;
const SWATCH_SIZE: f32 = 14.0;
const TAG_WIDTH: f32 = 98.0;
const PLAYER_ARROW_WIDTH: f32 = 20.0;
const COUNTRY_WIDTH: f32 = 162.0;
const DIFFICULTY_WIDTH: f32 = 50.0;
/// Padding inside the border (1 pixel) of the player list.
const PANEL_PADDING: f32 = 8.0;
/// Height of the DEPLOY, Options and Quit buttons, which stand side by side.
const START_HEIGHT: f32 = 32.0;

/// The width of a row of a column with a label and a value of the widths `widths`.
fn row_width((label, value): (f32, f32)) -> f32 {
    label + value + 2.0 * ARROW_WIDTH + 3.0 * CELL_GAP
}

/// The width of the player list: its longest line (a computer player's) in its padding and
/// border.
fn players_width() -> f32 {
    SWATCH_SIZE
        + TAG_WIDTH
        + COUNTRY_WIDTH
        + DIFFICULTY_WIDTH
        + 4.0 * PLAYER_ARROW_WIDTH
        + 7.0 * CELL_GAP
        + 2.0 * (PANEL_PADDING + 1.0)
}

/// The height of the player list: its title and a line for each of the players a match may
/// have, in its padding and border.
fn players_height() -> f32 {
    TITLE_HEIGHT
        + sim::MAX_MATCH_PLAYERS as f32 * (PLAYER_HEIGHT + ROW_GAP)
        + 2.0 * (PANEL_PADDING + 1.0)
}

/// The ground the menu covers, from its measures: the three columns side by side, under the
/// heading lines and over the buttons. It is the same in every window that holds it, and no
/// larger than [`MENU_FIT`] (a unit test; the smoke test measures the laid-out menu against
/// both).
pub fn menu_size() -> Vec2 {
    let rows = Row::MATCH.len().max(Row::RULES.len()) as f32;
    let column = TITLE_HEIGHT + rows * (ROW_HEIGHT + ROW_GAP);
    Vec2::new(
        row_width(MATCH_WIDTHS) + row_width(RULES_WIDTHS) + players_width() + 2.0 * COLUMN_GAP,
        HEADING_HEIGHTS.iter().sum::<f32>()
            + HEADING_HEIGHTS.len() as f32 * BLOCK_GAP
            + column.max(players_height())
            + BLOCK_GAP
            + START_HEIGHT,
    )
}

/// A column of menu rows under its title.
fn spawn_column(columns: &mut ChildSpawnerCommands, title: &str, rows: &[Row], widths: (f32, f32)) {
    columns
        .spawn((
            MenuBlock,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(ROW_GAP),
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .with_children(|column| {
            column.spawn((
                cell(row_width(widths), TITLE_HEIGHT, JustifyContent::Start),
                children![screen_text(title, 11.0)],
            ));
            for &row in rows {
                column
                    .spawn(Node {
                        column_gap: Val::Px(CELL_GAP),
                        align_items: AlignItems::Center,
                        flex_shrink: 0.0,
                        ..default()
                    })
                    .with_children(|line| {
                        line.spawn((
                            cell(widths.0, ROW_HEIGHT, JustifyContent::Start),
                            children![screen_text(row.label(), ROW_FONT)],
                        ));
                        line.spawn((
                            MenuButton::Previous(row),
                            screen_button(ARROW_WIDTH, ROW_HEIGHT),
                        ))
                        .with_child(screen_text("<", ROW_FONT));
                        line.spawn((
                            cell(widths.1, ROW_HEIGHT, JustifyContent::Center),
                            children![(ScreenText::Row(row), screen_text("", ROW_FONT))],
                        ));
                        line.spawn((
                            MenuButton::Next(row),
                            screen_button(ARROW_WIDTH, ROW_HEIGHT),
                        ))
                        .with_child(screen_text(">", ROW_FONT));
                    });
            }
        });
}

/// The `<`, value and `>` of `row` in a line of the player list.
fn spawn_player_row(line: &mut ChildSpawnerCommands, row: Row, width: f32) {
    line.spawn((
        MenuButton::Previous(row),
        screen_button(PLAYER_ARROW_WIDTH, PLAYER_HEIGHT),
    ))
    .with_child(screen_text("<", PLAYER_FONT));
    line.spawn((
        cell(width, PLAYER_HEIGHT, JustifyContent::Center),
        children![(ScreenText::Row(row), screen_text("", PLAYER_FONT))],
    ));
    line.spawn((
        MenuButton::Next(row),
        screen_button(PLAYER_ARROW_WIDTH, PLAYER_HEIGHT),
    ))
    .with_child(screen_text(">", PLAYER_FONT));
}

/// Spawns the menu (shown at first), the end screen and the list of the match's players (both
/// hidden), above the sidebar. The menu is the heading lines, three columns side by side (the
/// rows of the match, the rows of its rules, and the player list with a line for each of the
/// `sim::MAX_MATCH_PLAYERS` players a match may have, each with its country and, for a computer
/// player, its difficulty), and the DEPLOY, Options and Quit buttons in a row ([`menu_size`]).
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
                row_gap: Val::Px(BLOCK_GAP),
                ..default()
            },
            BackgroundColor(Color::srgb(0.055, 0.075, 0.078)),
            GlobalZIndex(10),
            FocusPolicy::Block,
        ))
        .with_children(|menu| {
            let [kicker, title, subtitle, rule, gallery, prompt] = HEADING_HEIGHTS;
            let heading = |height: f32| {
                (
                    MenuBlock,
                    Node {
                        height: Val::Px(height),
                        flex_shrink: 0.0,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                )
            };
            menu.spawn((
                heading(kicker),
                children![screen_text("FIELD OPERATIONS", 13.0)],
            ));
            menu.spawn((heading(title), children![screen_text("MyGame001", 44.0)]));
            menu.spawn((
                heading(subtitle),
                children![screen_text("SKIRMISH COMMAND", 13.0)],
            ));
            menu.spawn((
                MenuBlock,
                Node {
                    width: Val::Px(464.0),
                    height: Val::Px(rule),
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.55, 0.47, 0.25)),
            ));
            menu.spawn((
                MenuBlock,
                Node {
                    height: Val::Px(gallery),
                    flex_shrink: 0.0,
                    column_gap: Val::Px(12.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
            ))
            .with_children(|row| {
                let Some(sprites) = &sprites else {
                    return;
                };
                for kind in [0, 1, 4, 5] {
                    if let Some(image) = sprites.0.get(&sim::BuildingKind(kind)) {
                        row.spawn((
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
            menu.spawn((
                heading(prompt),
                children![screen_text("SELECT YOUR THEATER OF OPERATIONS", 11.0)],
            ));
            menu.spawn(Node {
                column_gap: Val::Px(COLUMN_GAP),
                align_items: AlignItems::Start,
                flex_shrink: 0.0,
                ..default()
            })
            .with_children(|columns| {
                spawn_column(columns, "MATCH", &Row::MATCH, MATCH_WIDTHS);
                spawn_column(columns, "RULES", &Row::RULES, RULES_WIDTHS);
                // Every player of the match with its colour, country and difficulty, so the
                // teams are clear before deploying.
                columns
                    .spawn((
                        MenuBlock,
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(ROW_GAP),
                            width: Val::Px(players_width()),
                            // As high as ten players need, however many the match has, so
                            // that the buttons below do not move with the enemy players row.
                            height: Val::Px(players_height()),
                            flex_shrink: 0.0,
                            padding: UiRect::all(Val::Px(PANEL_PADDING)),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BorderColor::all(Color::srgb(0.43, 0.46, 0.32)),
                        BackgroundColor(Color::srgb(0.08, 0.10, 0.10)),
                    ))
                    .with_children(|panel| {
                        panel.spawn((
                            cell(
                                players_width() - 2.0 * (PANEL_PADDING + 1.0),
                                TITLE_HEIGHT,
                                JustifyContent::Start,
                            ),
                            children![screen_text("PLAYERS", 11.0)],
                        ));
                        for slot in 0..sim::MAX_MATCH_PLAYERS {
                            panel
                                .spawn((
                                    PlayerLine(slot),
                                    Node {
                                        column_gap: Val::Px(CELL_GAP),
                                        align_items: AlignItems::Center,
                                        flex_shrink: 0.0,
                                        ..default()
                                    },
                                ))
                                .with_children(|line| {
                                    line.spawn((
                                        PlayerSwatch(slot),
                                        Node {
                                            width: Val::Px(SWATCH_SIZE),
                                            height: Val::Px(SWATCH_SIZE),
                                            flex_shrink: 0.0,
                                            border: UiRect::all(Val::Px(1.0)),
                                            ..default()
                                        },
                                        BorderColor::all(Color::srgb(0.9, 0.9, 0.9)),
                                        BackgroundColor(Color::BLACK),
                                    ));
                                    line.spawn((
                                        cell(TAG_WIDTH, PLAYER_HEIGHT, JustifyContent::Start),
                                        children![(
                                            ScreenText::Player(slot),
                                            screen_text("", PLAYER_FONT)
                                        )],
                                    ));
                                    spawn_player_row(line, Row::Country(slot), COUNTRY_WIDTH);
                                    if slot > 0 {
                                        spawn_player_row(
                                            line,
                                            Row::Difficulty(slot),
                                            DIFFICULTY_WIDTH,
                                        );
                                    }
                                });
                        }
                    });
            });
            menu.spawn((
                MenuBlock,
                Node {
                    column_gap: Val::Px(COLUMN_GAP),
                    flex_shrink: 0.0,
                    ..default()
                },
            ))
            .with_children(|buttons| {
                buttons
                    .spawn((MenuButton::Start, screen_button(200.0, START_HEIGHT)))
                    .with_child(screen_text("DEPLOY", 20.0));
                buttons
                    .spawn((MenuButton::Options, screen_button(200.0, START_HEIGHT)))
                    .with_child(screen_text("Options", 18.0));
                buttons
                    .spawn((MenuButton::Quit, screen_button(200.0, START_HEIGHT)))
                    .with_child(screen_text("Quit", 18.0));
            });
        });
    commands
        .spawn((
            EndRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(crate::sidebar::SIDEBAR_WIDTH),
                top: Val::Percent(24.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                display: Display::None,
                ..default()
            },
            GlobalZIndex(9),
        ))
        .with_children(|end| {
            end.spawn((ScreenText::Outcome, screen_text("", 56.0)));
            end.spawn((ScreenText::Reason, screen_text("", 15.0)));
            end.spawn((EndButton::Restart, screen_button(200.0, START_HEIGHT)))
                .with_child(screen_text("Restart", 20.0));
            end.spawn((EndButton::Menu, screen_button(200.0, START_HEIGHT)))
                .with_child(screen_text("Main menu", 20.0));
        });
    // The players of the running match, below the PAUSED banner and the end screen.
    commands
        .spawn((
            PlayedRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(crate::sidebar::SIDEBAR_WIDTH),
                top: Val::Percent(56.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                display: Display::None,
                ..default()
            },
            GlobalZIndex(9),
            FocusPolicy::Pass,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(3.0),
                    padding: UiRect::all(Val::Px(PANEL_PADDING)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BorderColor::all(Color::srgb(0.43, 0.46, 0.32)),
                BackgroundColor(Color::srgba(0.03, 0.04, 0.05, 0.84)),
            ))
            .with_children(|panel| {
                panel.spawn(screen_text("PLAYERS", 11.0));
                for slot in 0..sim::MAX_MATCH_PLAYERS {
                    panel
                        .spawn((
                            PlayedLine(slot),
                            Node {
                                column_gap: Val::Px(CELL_GAP),
                                align_items: AlignItems::Center,
                                display: Display::None,
                                ..default()
                            },
                        ))
                        .with_children(|line| {
                            line.spawn((
                                PlayedSwatch(slot),
                                Node {
                                    width: Val::Px(SWATCH_SIZE),
                                    height: Val::Px(SWATCH_SIZE),
                                    flex_shrink: 0.0,
                                    border: UiRect::all(Val::Px(1.0)),
                                    ..default()
                                },
                                BorderColor::all(Color::srgb(0.9, 0.9, 0.9)),
                                BackgroundColor(Color::BLACK),
                            ));
                            line.spawn((ScreenText::Played(slot), screen_text("", 13.0)));
                        });
                }
            });
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

/// Shows the menu, the end screen or the list of the match's players (while the match is paused
/// or over), with their texts.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn update_screens(
    screen: Res<Screen>,
    selection: Res<MenuSelection>,
    catalogue: Option<Res<Catalogue>>,
    host: Res<SimHost>,
    sink: Res<crate::sim_host::CommandSink>,
    played: Option<Res<MatchPlayers>>,
    teams: Option<Res<crate::presentation::TeamColors>>,
    mut roots: Query<
        (&mut Node, Has<MenuRoot>, Has<EndRoot>),
        Or<(With<MenuRoot>, With<EndRoot>, With<PlayedRoot>)>,
    >,
    mut texts: Query<(&mut Text, &ScreenText)>,
) {
    let outcome = end_text(host.outcome(), host.human_defeated());
    let rules = host.match_state().map(sim::MatchState::rules);
    let reason = end_reason(
        host.outcome(),
        host.human_defeated(),
        rules.is_none_or(|rules| rules.short_game),
    );
    let in_match = *screen == Screen::Match;
    for (mut node, menu, end) in &mut roots {
        let shown = if menu {
            *screen == Screen::Menu
        } else if end {
            in_match && outcome.is_some()
        } else {
            in_match && (outcome.is_some() || sink.paused())
        };
        let display = if shown { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    }
    let colours = teams.map_or(selection.color, |teams| teams.0);
    for (mut text, kind) in &mut texts {
        let value = match kind {
            ScreenText::Row(row) => catalogue.as_deref().map_or_else(String::new, |catalogue| {
                row_text(*row, *selection, catalogue)
            }),
            ScreenText::Outcome => outcome.unwrap_or_default().to_string(),
            ScreenText::Reason => reason.unwrap_or_default().to_string(),
            ScreenText::Player(slot) => player_tag(*slot, *selection).unwrap_or_default(),
            ScreenText::Played(slot) => played
                .as_deref()
                .and_then(|played| {
                    let defeated = played.0.get(*slot).is_some_and(|entry| {
                        host.match_state()
                            .is_some_and(|state| state.defeated(entry.player).is_some())
                    });
                    played_line(*slot, played, colours, defeated)
                })
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
        assert_eq!(buttons.iter(app.world()).count(), Row::COUNT * 2 + 3);
        // A `<` and a `>` for every row, each row once.
        let rows: BTreeSet<Row> = Row::all().collect();
        assert_eq!((rows.len(), Row::all().count()), (Row::COUNT, Row::COUNT));
        for row in rows {
            for button in [
                super::MenuButton::Previous(row),
                super::MenuButton::Next(row),
            ] {
                assert_eq!(
                    buttons
                        .iter(app.world())
                        .filter(|&&other| other == button)
                        .count(),
                    1,
                    "{button:?}"
                );
            }
        }
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

    /// The directory of the legacy ruleset fixture: the data files of the game as they were
    /// before RA1 (the Vanguard and the Meridian, factions without a side that start with their
    /// buildings and groups and build by their rosters), in the schemas of today. It has no
    /// rules file.
    pub(crate) fn legacy_data() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sim/tests/fixtures/data/legacy")
    }

    /// The catalogue of the legacy ruleset fixture ([`legacy_data`]) with the sprites and maps
    /// of the game, for the tests of what only factions without a side have.
    pub(crate) fn legacy_catalogue() -> Catalogue {
        load_ruleset(&assets(), &legacy_data(), false)
            .unwrap_or_else(|error| panic!("{error}"))
            .0
    }

    /// [`load`] of a ruleset whose data files lie in the directory `data`, with the sprites and
    /// maps of the asset directory `root`; `rules` says whether it has a rules file, which the
    /// legacy ruleset fixture has not (the game itself always loads one).
    fn load_ruleset(root: &Path, data: &Path, rules: bool) -> Result<Loaded, String> {
        let loaded = crate::units::load_ruleset_data(root, data, rules)
            .map_err(|error| error.to_string())?;
        catalogue_of(root, data, loaded, &CheckedAssets::default())
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
        check_catalogue(&catalogue, &root.join(crate::units::DATA_DIR))
            .unwrap_or_else(|error| panic!("{error}"));
        // RA1 content: the nine countries of both sides, each a country of the rules file with
        // a tech tree of its own (its roster), three maps, the full roster.
        let factions = &catalogue.skirmish.factions;
        assert_eq!(factions.len(), 9);
        let rules = catalogue.definitions.rules().expect("rules");
        for faction in factions {
            let country = faction.country.and_then(|country| rules.country(country));
            assert_eq!(
                country.map(|country| (country.id.as_str(), Some(country.side))),
                Some((faction.id.as_str(), faction.side)),
                "{}",
                faction.id
            );
        }
        for side in sim::Side::ALL {
            assert!(factions.iter().any(|faction| faction.side == Some(side)));
        }
        let rosters: BTreeSet<&BTreeSet<sim::ProductionItem>> =
            factions.iter().map(|faction| &faction.roster).collect();
        assert_eq!(rosters.len(), factions.len(), "the rosters differ");
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
            crate::units::RULES_PATH,
            crate::units::ARSENAL_PATH,
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
        // Every sprite is named by a definition: of the shipped data, or (RA1) of the legacy
        // ruleset fixture, which the tests load with the game's sprites (`legacy_catalogue`), so
        // the sprites of the one type that only it still shows, the rocket tower, stay in the
        // assets.
        let legacy = legacy_catalogue();
        let named = |definitions: &Definitions, file: &Path, stem: &str| {
            let hd_used = if file.starts_with("hd/buildings") {
                definitions.buildings().iter().any(|d| d.id == stem)
            } else if file.starts_with("hd/units") || file.starts_with("hd/walk") {
                // A walk cycle belongs to the unit whose sheet has its name.
                definitions.units().iter().any(|d| d.id == stem)
            } else {
                // The terrain atlases battlefield.rs loads, and the particle textures.
                (0..crate::battlefield::ATLASES)
                    .any(|i| file == Path::new(&format!("hd/terrain/{i}.png")))
                    || ["hd/glow.png", "hd/smoke.png", "hd/ring.png", "hd/crate.png"]
                        .iter()
                        .any(|p| file == Path::new(p))
            };
            hd_used
                || definitions.units().iter().any(|unit| unit.sprite == stem)
                || definitions
                    .buildings()
                    .iter()
                    .any(|building| building.sprite == stem)
        };
        let mut legacy_only = Vec::new();
        for file in files_under(&root.join("sprites")) {
            let stem = file
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("name");
            if named(&catalogue.definitions, &file, stem) {
                continue;
            }
            assert!(
                named(&legacy.definitions, &file, stem),
                "sprites/{} is named by no definition",
                file.display()
            );
            legacy_only.push(file);
        }
        assert_eq!(
            legacy_only,
            [
                "buildings/rocket_tower.png",
                "hd/buildings/rocket_tower.png"
            ]
            .map(PathBuf::from)
        );
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
        random: true,
        difficulties: 3,
        starts: {
            let mut starts = [0; sim::MAX_MAPS];
            starts[0] = 2;
            starts[1] = 4;
            starts
        },
        lobby: Lobby {
            min_credits: 5000,
            max_credits: 10000,
            credits_step: 100,
            credits: 10000,
            min_unit_count: 0,
            max_unit_count: 10,
            unit_count: 10,
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
        // The menu starts with the middle difficulty, for every computer player.
        let selection = MenuSelection::initial(&catalogue);
        assert_eq!(
            selection.difficulties,
            [catalogue.ai.difficulties.len() / 2; sim::MAX_MATCH_PLAYERS]
        );
        assert!(selection.valid(MenuCounts::of(&catalogue)));
    }

    #[test]
    fn every_shipped_match_can_be_set_up_for_every_faction_pairing() {
        // Cross validation of the shipped data: every faction fits at both start positions of
        // every map, against every other faction (both orders) and with every difficulty. Since
        // RA1 a faction is a country, which starts with the base unit of its side and its
        // starting units instead of a yard and groups (`starting_totals`).
        let catalogue = production_catalogue();
        let counts = MenuCounts::of(&catalogue);
        for map in 0..counts.maps {
            for faction in 0..counts.factions {
                for opponent in 0..counts.factions {
                    for difficulty in 0..counts.difficulties {
                        let selection = MenuSelection {
                            map,
                            difficulties: [difficulty; sim::MAX_MATCH_PLAYERS],
                            ..MenuSelection::default()
                        }
                        .with_countries(faction, opponent, counts.factions);
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
                        let (expected_units, expected_buildings) =
                            selection_totals(&catalogue, selection).expect("fixed countries");
                        assert!(expected_units > 2, "{selection:?}");
                        assert_eq!(start.units, expected_units);
                        assert_eq!(start.buildings, expected_buildings);
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
        //
        // Since RA1 the shipped factions are countries, which have no yard and no groups in the
        // skirmish file, so the root holds the legacy ruleset fixture (`legacy_data`: the data
        // files the edits below were written for, without a rules file) with the game's maps and
        // sprites.
        let load = |root: &Path| load_ruleset(root, &root.join(crate::units::DATA_DIR), false);
        let root = PathBuf::from(std::env::var_os("HOME").expect("HOME for review fixtures"))
            .join("Library/Caches/Codex/reviews/MyGame001/rts-controls")
            .join(format!("skirmish-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        copy_dir(&legacy_data(), &root.join(crate::units::DATA_DIR));
        for dir in ["maps", "sprites"] {
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
        let error = load(&root)
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
        let error = load(&root)
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
        load(&root).unwrap_or_else(|error| panic!("{error}"));
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
        //
        // The checks of the start positions run on the shipped data (RA1: the nine countries)
        // and on the legacy ruleset fixture; those of a faction's starting buildings on the
        // fixture, whose factions have them, and on the shipped data what a country can break.
        let catalogue = production_catalogue();
        let root = assets();
        let data = root.join(crate::units::DATA_DIR);
        check_catalogue(&catalogue, &data).unwrap_or_else(|error| panic!("{error}"));
        let skirmish_file = format!("data file {}", root.join(SKIRMISH_PATH).display());
        let legacy = legacy_catalogue();
        let legacy_dir = legacy_data();
        check_catalogue(&legacy, &legacy_dir).unwrap_or_else(|error| panic!("{error}"));
        let legacy_file = format!("data file {}", legacy_dir.join("skirmish.ron").display());

        for (catalogue, data, skirmish_file) in [
            (&catalogue, &data, &skirmish_file),
            (&legacy, &legacy_dir, &legacy_file),
        ] {
            let mut outside = catalogue.clone();
            outside.skirmish.maps[0].starts[1] = sim::CellPos::new(70, 42);
            for building in &mut outside.skirmish.factions[0].buildings {
                building.offset.0 -= 18;
            }
            for group in &mut outside.skirmish.factions[0].units {
                group.offset.0 -= 18;
            }
            let error = check_catalogue(&outside, data).expect_err("a start outside the map");
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
            let error = check_catalogue(&on_rock, data).expect_err("a start on rock");
            assert!(error.contains("maps[0].starts[0]"), "{error}");
            assert!(error.contains("impassable terrain"), "{error}");
        }

        // An AI file whose build order needs a producer that a faction does not start with.
        let mut no_yard = legacy.clone();
        let power_plant = no_yard
            .definitions
            .building_kind("power_plant")
            .expect("power plant");
        no_yard.skirmish.factions[0].buildings[0].kind = power_plant;
        let error = check_catalogue(&no_yard, &legacy_dir).expect_err("no yard");
        for part in [
            format!("data file {}", legacy_dir.join("ai.ron").display()),
            "build_order[0]".to_string(),
            "whose producer \"construction_yard\" faction \"vanguard\"".to_string(),
        ] {
            assert!(error.contains(&part), "{error:?} lacks {part:?}");
        }
        // A country always has the yard its base unit deploys into; the AI file fails it
        // without a plan for its side.
        let mut no_plan = catalogue.clone();
        no_plan
            .ai
            .plans
            .retain(|plan| plan.side != Some(sim::Side::Soviet));
        let error = check_catalogue(&no_plan, &data).expect_err("no plan");
        for part in [
            format!("data file {}", root.join(AI_PATH).display()),
            "plans".to_string(),
            "plan for the side of faction \"libya\" \"soviet\" has no entry".to_string(),
        ] {
            assert!(error.contains(&part), "{error:?} lacks {part:?}");
        }

        // A faction whose buildings do not fit at a start of a map: the match cannot be set up.
        let mut too_far = legacy.clone();
        too_far.skirmish.factions[0].buildings[0].offset = (32, 0);
        let error = check_catalogue(&too_far, &legacy_dir).expect_err("off the map");
        for part in [
            legacy_file.as_str(),
            "map \"demo\": the match of faction \"vanguard\" against faction \"vanguard\"",
            "starting building \"construction_yard\"",
        ] {
            assert!(error.contains(part), "{error:?} lacks {part:?}");
        }
        // Likewise a country with a starting building (which match setup places besides its
        // base unit) off the map.
        let mut too_far = catalogue.clone();
        let yard = too_far
            .definitions
            .building_kind("construction_yard")
            .expect("yard");
        too_far.skirmish.factions[0]
            .buildings
            .push(sim::StartingBuilding {
                kind: yard,
                offset: (64, 0),
            });
        let error = check_catalogue(&too_far, &data).expect_err("off the map");
        for part in [
            skirmish_file.as_str(),
            "map \"demo\": the match of faction \"america\" against faction",
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
        // of a match grows with its players and gets the start positions it lacks. (Since RA1
        // with the menu's MAP GROWTH row on, as it is throughout this test; the row is off at
        // first, docs/DECISIONS.md R4, and
        // `map_growth_is_an_option_and_off_limits_the_enemies_to_the_starts_of_the_file` covers
        // that.)
        assert_eq!(catalogue.skirmish.maps[riverlands].starts.len(), 4);
        for map in 0..counts.maps {
            assert_eq!(counts.enemies(map, true), sim::MAX_MATCH_PLAYERS - 1);
        }
        assert_eq!(counts.enemies(counts.maps, true), 0, "no such map");
        // Next on the row: 1, 2, ... 9, then back to 1. (The countries are fixed, so that what
        // a match starts with is known before it is set up: `MenuSelection::versus`.)
        let mut selection = MenuSelection {
            map: demo,
            map_growth: true,
            ..MenuSelection::versus(&catalogue)
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
            // Since RA1 every player starts with the base unit of its side and its starting
            // units, and with the lobby's "start with an MCV" off with the yard it deploys into.
            let start = crate::sim_host::check_match(&plan).expect("the match sets up");
            assert_eq!(
                Some((start.units, start.buildings)),
                selection_totals(&catalogue, choice)
            );
            assert_eq!(start.buildings, 0, "a base unit each, no yard yet");
            let yards = MatchPlan {
                options: sim::MatchOptions {
                    mcv_start: false,
                    ..plan.options.clone()
                },
                ..plan.clone()
            };
            let start = crate::sim_host::check_match(&yards).expect("the match sets up");
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
            .filter_map(|slot| player_tag(slot, selection))
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
                .filter_map(|slot| player_tag(slot, full))
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
        // RA1: a country row has the three factions and then RANDOM. The human player's starts
        // on the first faction, so the entry before it is RANDOM and the one before that the
        // last faction; a computer player's starts on RANDOM, so the next entry is the first
        // faction.
        let previous =
            |selection, row| match menu_effect(MenuButton::Previous(row), selection, COUNTS) {
                MenuEffect::Select(next) => next,
                other => panic!("{other:?}"),
            };
        let countries = |human, computer| {
            let mut countries = start.countries;
            (countries[0], countries[1]) = (human, computer);
            MenuSelection { countries, ..start }
        };
        let random = previous(start, Row::Country(0));
        assert_eq!(random, countries(None, None));
        assert_eq!(
            previous(random, Row::Country(0)),
            countries(Some(2), None),
            "the last faction"
        );
        assert_eq!(next(random, Row::Country(0)), start, "wraps");
        assert_eq!(
            next(start, Row::Country(1)),
            countries(Some(0), Some(0)),
            "from RANDOM to the first faction"
        );
        // RA1: a difficulty row for every computer player.
        let mut difficulties = start.difficulties;
        difficulties[1] = 2;
        assert_eq!(
            menu_effect(MenuButton::Previous(Row::Difficulty(1)), start, COUNTS),
            MenuEffect::Select(MenuSelection {
                difficulties,
                ..start
            })
        );
        // The human player has none, and a slot beyond the players a match may have has no row.
        for row in [
            Row::Difficulty(0),
            Row::Difficulty(sim::MAX_MATCH_PLAYERS),
            Row::Country(sim::MAX_MATCH_PLAYERS),
        ] {
            assert_eq!(
                menu_effect(MenuButton::Next(row), start, COUNTS),
                MenuEffect::Nothing,
                "{row:?}"
            );
        }
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
            menu_effect(MenuButton::Next(Row::Difficulty(1)), start, empty),
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
            difficulties: [1; sim::MAX_MATCH_PLAYERS],
            ..MenuSelection::default()
        }
        .with_countries(1, 2, 3);
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
            difficulties: [2; sim::MAX_MATCH_PLAYERS],
            ..MenuSelection::default()
        }
        .with_countries(0, 1, 3);
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
        // (Since RA1 a row per computer player, and the id is shown with a capital.)
        let difficulty = row_text(Row::Difficulty(1), selection, &catalogue);
        assert_eq!(
            difficulty.to_lowercase(),
            catalogue.ai.difficulties[selection.difficulties[1]]
                .id
                .replace('_', " ")
        );
        assert_eq!(difficulty, "Medium");
        let outside = MenuSelection {
            super_me: false,
            color: 0,
            map: catalogue.maps.len(),
            ..selection
        };
        assert_eq!(row_text(Row::Map, outside, &catalogue), "-");
    }

    // --- RA1: the lobby of Red Alert 2 in the menu ----------------------------------------------

    /// The selection after one press of the `>` (`forward`) or `<` button of `row`.
    fn stepped(
        selection: MenuSelection,
        row: Row,
        forward: bool,
        counts: MenuCounts,
    ) -> MenuSelection {
        let button = if forward {
            MenuButton::Next(row)
        } else {
            MenuButton::Previous(row)
        };
        match menu_effect(button, selection, counts) {
            MenuEffect::Select(next) => next,
            other => panic!("{row:?}: {other:?}"),
        }
    }

    /// The match of `selection` set up as the menu starts it, with the countries its players
    /// play.
    fn set_up(
        catalogue: &Catalogue,
        selection: MenuSelection,
        seed: u64,
    ) -> (SimHost, sim::MatchStart, MatchPlayers) {
        let (host, start) = crate::sim_host::tests::match_host(catalogue, selection, seed);
        let played = MatchPlayers::of(&host, catalogue, &selection.players());
        (host, start, played)
    }

    #[test]
    fn the_menu_opens_with_the_lobby_of_red_alert_2() {
        // RA1 (docs/RA2_PARITY_PLAN.md section 3.5): the default match is that of Red Alert 2's
        // lobby, with this game's extensions off.
        let catalogue = production_catalogue();
        let counts = MenuCounts::of(&catalogue);
        let selection = MenuSelection::initial(&catalogue);
        assert!(selection.valid(counts));
        // What the simulation is set up from: its own default options, with the one computer
        // player's country to be drawn.
        assert_eq!(
            selection.options(),
            sim::MatchOptions {
                random_countries: vec![PlayerId(1)],
                // (The default's 0 means 100 too: nobody's units are faster.)
                unit_speed_percent: 100,
                ..sim::MatchOptions::default()
            }
        );
        let medium = catalogue.ai.difficulties.len() / 2;
        assert_eq!(
            selection.players(),
            [
                PlayerSetup {
                    player: HUMAN,
                    faction: 0,
                    start: 0,
                    controller: Controller::Human,
                },
                PlayerSetup {
                    player: PlayerId(1),
                    faction: 0,
                    start: 1,
                    controller: Controller::Ai { difficulty: medium },
                },
            ]
        );
        // The lobby is that of the rules file, and the rows show its defaults.
        let dialog = catalogue.definitions.rules().expect("rules").dialog;
        assert_eq!(
            counts.lobby,
            Lobby {
                min_credits: i64::from(dialog.min_credits),
                max_credits: i64::from(dialog.max_credits),
                credits_step: i64::from(dialog.credits_step),
                credits: catalogue.skirmish.starting_credits,
                min_unit_count: dialog.min_unit_count as u32,
                max_unit_count: dialog.max_unit_count as u32,
                unit_count: dialog.unit_count as u32,
            }
        );
        assert_eq!(
            i64::from(dialog.credits),
            catalogue.skirmish.starting_credits
        );
        let text = |row| row_text(row, selection, &catalogue);
        assert_eq!(text(Row::Country(0)), "America (Allied)");
        assert_eq!(text(Row::Country(1)), "RANDOM");
        assert_eq!(text(Row::Difficulty(1)), "Medium");
        assert_eq!(text(Row::Credits), dialog.credits.to_string());
        assert_eq!(text(Row::UnitCount), dialog.unit_count.to_string());
        for (row, on) in [
            (Row::McvStart, dialog.bases),
            (Row::ShortGame, dialog.short_game),
            (Row::Crates, dialog.crates),
            (Row::McvRepacks, dialog.mcv_redeploys),
            (Row::Superweapons, 1),
        ] {
            assert_eq!(text(row), "ON", "{row:?}");
            assert_eq!(on, 1, "{row:?}: the lobby of the rules file");
        }
        assert_eq!(text(Row::TeamMode), "ALLIED AI");
        assert_eq!(text(Row::MapGrowth), "OFF");
        assert_eq!(text(Row::SuperMe), "OFF");
        assert_eq!(text(Row::UnitSpeed), "1x - SUPER ME OFF");
        assert_eq!(text(Row::UnlockAll), "STANDARD");
        assert_eq!(
            text(Row::Enemies),
            "1 of 5",
            "the demo map lists six starts"
        );
        // A row outside the menu has no text.
        assert_eq!(text(Row::Country(sim::MAX_MATCH_PLAYERS)), "-");
        assert_eq!(text(Row::Difficulty(0)), "-");
        // The match it starts: the credits and the rules of the lobby, an MCV each.
        let (host, start, played) = set_up(&catalogue, selection, 3);
        assert_eq!(start.buildings, 0);
        assert_eq!(
            (start.units, start.buildings),
            starting_totals(
                &catalogue,
                played.factions(),
                dialog.unit_count as u32,
                true
            )
        );
        assert_eq!(
            host.match_state().map(sim::MatchState::rules),
            Some(sim::MatchRules::default())
        );
        let players = host.players().expect("players");
        for id in [HUMAN, PlayerId(1)] {
            assert_eq!(
                players.get(id).map(sim::PlayerState::credits),
                Some(catalogue.skirmish.starting_credits)
            );
        }
    }

    #[test]
    fn the_credits_and_unit_count_rows_cover_their_ranges_and_reach_the_match() {
        // RA1 item 5: both ends of every range, through `MenuSelection::options` into the
        // simulation. The ranges are those of the rules file.
        let catalogue = production_catalogue();
        let counts = MenuCounts::of(&catalogue);
        let lobby = counts.lobby;
        // Fixed countries, so that what the match starts with is known.
        let initial = MenuSelection::versus(&catalogue);
        let defaults = initial.options();
        assert_eq!(
            (defaults.starting_credits, defaults.unit_count),
            (None, None)
        );
        let (low, high, step) = (lobby.min_credits, lobby.max_credits, lobby.credits_step);
        assert!(low < high && step > 0 && (high - low) % step == 0);
        assert_eq!(lobby.credit_amounts() as i64, (high - low) / step + 1);

        // Credits: `>` goes up a step at a time and wraps from the highest to the lowest.
        let mut selection = initial;
        let mut amounts = Vec::new();
        for _ in 0..lobby.credit_amounts() {
            let before = selection.credits.unwrap_or(lobby.credits);
            selection = stepped(selection, Row::Credits, true, counts);
            let credits = selection.credits.expect("chosen");
            assert_eq!(credits, if before == high { low } else { before + step });
            assert!(selection.valid(counts) && lobby.offers_credits(credits));
            assert_eq!(
                selection.options(),
                sim::MatchOptions {
                    starting_credits: Some(credits),
                    ..defaults.clone()
                },
                "the row changes the credits and nothing else"
            );
            assert_eq!(
                row_text(Row::Credits, selection, &catalogue),
                credits.to_string()
            );
            amounts.push(credits);
        }
        amounts.sort_unstable();
        assert_eq!(
            amounts,
            (0..lobby.credit_amounts() as i64)
                .map(|k| low + k * step)
                .collect::<Vec<_>>()
        );
        // `<` from the lowest wraps to the highest, and from the highest goes down a step.
        let lowest = MenuSelection {
            credits: Some(low),
            ..initial
        };
        let highest = stepped(lowest, Row::Credits, false, counts);
        assert_eq!(highest.credits, Some(high));
        assert_eq!(
            stepped(highest, Row::Credits, false, counts).credits,
            Some(high - step)
        );
        assert_eq!(
            stepped(highest, Row::Credits, true, counts).credits,
            Some(low)
        );
        // Both ends start a match in which every player has that amount.
        for credits in [low, high] {
            let choice = MenuSelection {
                credits: Some(credits),
                ..initial
            };
            let (host, ..) = set_up(&catalogue, choice, 0);
            for (_, state) in host.players().expect("players").iter() {
                assert_eq!(state.credits(), credits);
            }
        }
        // An amount outside the range or off the step is no entry of the row: the selection is
        // not valid and Start does nothing (the buttons cannot make it; the simulation would
        // reject it, `ra1_match.rs`).
        for credits in [low - step, high + step, low + 1, -1] {
            let outside = MenuSelection {
                credits: Some(credits),
                ..initial
            };
            assert!(!lobby.offers_credits(credits), "{credits}");
            assert!(!outside.valid(counts), "{credits}");
            assert_eq!(
                menu_effect(MenuButton::Start, outside, counts),
                MenuEffect::Nothing
            );
        }

        // The unit count: every count from the lowest to the highest.
        let (fewest, most) = (lobby.min_unit_count, lobby.max_unit_count);
        assert!(fewest < most);
        assert_eq!(lobby.unit_counts() as u32, most - fewest + 1);
        let mut selection = initial;
        let mut seen = Vec::new();
        for _ in 0..lobby.unit_counts() {
            let before = selection.unit_count.unwrap_or(lobby.unit_count);
            selection = stepped(selection, Row::UnitCount, true, counts);
            let count = selection.unit_count.expect("chosen");
            assert_eq!(count, if before == most { fewest } else { before + 1 });
            assert!(selection.valid(counts));
            assert_eq!(
                selection.options(),
                sim::MatchOptions {
                    unit_count: Some(count),
                    ..defaults.clone()
                }
            );
            assert_eq!(
                row_text(Row::UnitCount, selection, &catalogue),
                count.to_string()
            );
            seen.push(count);
        }
        seen.sort_unstable();
        assert_eq!(seen, (fewest..=most).collect::<Vec<_>>());
        let least = MenuSelection {
            unit_count: Some(fewest),
            ..initial
        };
        assert_eq!(
            stepped(least, Row::UnitCount, false, counts).unit_count,
            Some(most)
        );
        // Both ends start a match with the starting units of that count, which for a count of
        // 0 is the base unit alone.
        let mut totals = Vec::new();
        for count in [fewest, most] {
            let choice = MenuSelection {
                unit_count: Some(count),
                ..initial
            };
            let (_, start, played) = set_up(&catalogue, choice, 0);
            assert_eq!(
                (start.units, start.buildings),
                starting_totals(&catalogue, played.factions(), count, true),
                "unit count {count}"
            );
            assert_eq!(
                Some((start.units, start.buildings)),
                selection_totals(&catalogue, choice)
            );
            totals.push(start.units);
        }
        assert_eq!(fewest, 0, "the lobby of Red Alert 2");
        assert_eq!(totals[0], 2, "unit count 0: an MCV each and nothing else");
        assert!(totals[1] > totals[0], "{totals:?}");
        let outside = MenuSelection {
            unit_count: Some(most + 1),
            ..initial
        };
        assert!(!lobby.offers_unit_count(most + 1) && !outside.valid(counts));
        assert_eq!(
            menu_effect(MenuButton::Start, outside, counts),
            MenuEffect::Nothing
        );
    }

    #[test]
    fn every_on_off_row_sets_its_option_of_the_match_and_no_other() {
        let catalogue = production_catalogue();
        let counts = MenuCounts::of(&catalogue);
        // Two computer players, so that the team mode shows; fixed countries.
        let initial = MenuSelection {
            enemies: 1,
            ..MenuSelection::versus(&catalogue)
        };
        let defaults = initial.options();
        type Field = fn(&mut sim::MatchOptions) -> &mut bool;
        let rows: [(Row, Field, &str, &str); 8] = [
            (Row::McvStart, |o| &mut o.mcv_start, "ON", "OFF"),
            (Row::ShortGame, |o| &mut o.short_game, "ON", "OFF"),
            (Row::Superweapons, |o| &mut o.superweapons, "ON", "OFF"),
            (Row::Crates, |o| &mut o.crates, "ON", "OFF"),
            (Row::FogOfWar, |o| &mut o.fog_of_war, "OFF", "ON"),
            (Row::OreGrowth, |o| &mut o.ore_grows, "ON", "OFF"),
            (Row::McvRepacks, |o| &mut o.mcv_repacks, "ON", "OFF"),
            (
                Row::TeamMode,
                |o| &mut o.free_for_all,
                "ALLIED AI",
                "FREE FOR ALL",
            ),
        ];
        for (row, field, at_first, changed) in rows {
            assert_eq!(row_text(row, initial, &catalogue), at_first, "{row:?}");
            // Two entries: `<` and `>` both change it, and again change it back.
            let other = stepped(initial, row, true, counts);
            assert_eq!(other, stepped(initial, row, false, counts), "{row:?}");
            assert_eq!(stepped(other, row, true, counts), initial, "{row:?}");
            assert_eq!(row_text(row, other, &catalogue), changed, "{row:?}");
            let mut expected = defaults.clone();
            let value = field(&mut expected);
            *value = !*value;
            assert_eq!(other.options(), expected, "{row:?}");
            assert_eq!(other.players(), initial.players(), "{row:?}");
            // In the match: the rules, the teams and what the players start with.
            let (host, start, played) = set_up(&catalogue, other, 0);
            assert_eq!(
                host.match_state().map(sim::MatchState::rules),
                Some(sim::MatchRules {
                    short_game: expected.short_game,
                    superweapons: expected.superweapons,
                    mcv_repacks: expected.mcv_repacks,
                    crates: expected.crates,
                    ore_grows: expected.ore_grows,
                    bridge_destruction: true,
                    fog_of_war: expected.fog_of_war,
                }),
                "{row:?}"
            );
            let players = host.players().expect("players");
            let (first, second) = (PlayerId(1), PlayerId(2));
            assert!(players.hostile(HUMAN, first) && players.hostile(HUMAN, second));
            assert_eq!(
                players.hostile(first, second),
                expected.free_for_all,
                "{row:?}"
            );
            assert_eq!(
                start.buildings,
                if expected.mcv_start { 0 } else { 3 },
                "{row:?}: a Construction Yard each without the MCV start"
            );
            assert_eq!(
                Some((start.units, start.buildings)),
                selection_totals(&catalogue, other),
                "{row:?}"
            );
            assert_eq!(played.0.len(), 3);
        }
        // The extensions of this game: off at first, and the human player's alone.
        for (row, at_first, changed) in [
            (Row::SuperMe, "OFF", "ON / 10x / $1"),
            (Row::UnlockAll, "STANDARD", "ALL UNLOCKED"),
        ] {
            assert_eq!(row_text(row, initial, &catalogue), at_first);
            let other = stepped(initial, row, true, counts);
            assert_eq!(row_text(row, other, &catalogue), changed);
            let options = other.options();
            assert_eq!(
                (
                    options.super_players.clone(),
                    options.unlock_players.clone()
                ),
                if row == Row::SuperMe {
                    (vec![HUMAN], Vec::new())
                } else {
                    (Vec::new(), vec![HUMAN])
                }
            );
            assert_eq!(
                sim::MatchOptions {
                    super_players: Vec::new(),
                    unlock_players: Vec::new(),
                    ..options
                },
                defaults
            );
        }
        // The unit speed row: every speed, in effect with Super Me only.
        let mut selection = MenuSelection {
            super_me: true,
            ..initial
        };
        let mut speeds = Vec::new();
        for _ in 0..UNIT_SPEEDS.len() {
            speeds.push(selection.options().unit_speed_percent);
            assert_eq!(
                MenuSelection {
                    super_me: false,
                    ..selection
                }
                .options()
                .unit_speed_percent,
                100
            );
            selection = stepped(selection, Row::UnitSpeed, true, counts);
        }
        assert_eq!(speeds, UNIT_SPEEDS);
        assert_eq!(selection.unit_speed, 0, "wraps");
        assert_eq!(
            stepped(selection, Row::UnitSpeed, false, counts).unit_speed,
            UNIT_SPEEDS.len() - 1
        );
        // The map row and the enemy players row at both ends: the map of the plan, and the
        // players of the match with their ids and start positions in the order of the panel.
        let last_map = stepped(initial, Row::Map, false, counts);
        assert_eq!(last_map.map, counts.maps - 1);
        let (plan, _) = match_plan(&catalogue, last_map, 0).expect("a plan");
        assert_eq!((plan.map, plan.players.len()), (counts.maps - 1, 3));
        assert_eq!(stepped(last_map, Row::Map, true, counts).map, 0, "wraps");
        let fewest = MenuSelection {
            map_growth: true,
            enemies: 0,
            ..initial
        };
        let most = stepped(fewest, Row::Enemies, false, counts);
        assert_eq!(fewest.players().len(), sim::MIN_MATCH_PLAYERS);
        assert_eq!(most.players().len(), sim::MAX_MATCH_PLAYERS);
        assert_eq!(stepped(most, Row::Enemies, true, counts), fewest, "wraps");
        for (slot, setup) in most.players().iter().enumerate() {
            assert_eq!((setup.player, setup.start), (PlayerId(slot as u8), slot));
            assert_eq!(setup.controller == Controller::Human, slot == 0);
        }
        // The team colour and the game speed belong to the game: the simulation is set up the
        // same whatever they are.
        for row in [Row::Color, Row::GameSpeed] {
            let (_, entries) = initial.position(row, counts);
            let mut selection = initial;
            for _ in 0..entries {
                selection = stepped(selection, row, true, counts);
                assert_eq!(selection.options(), defaults, "{row:?}");
                assert_eq!(selection.players(), initial.players(), "{row:?}");
            }
            assert_eq!(selection, initial, "{row:?} wraps");
            assert_ne!(stepped(initial, row, false, counts), initial, "{row:?}");
        }
    }

    #[test]
    fn country_and_difficulty_rows_set_each_player_of_the_match() {
        let catalogue = production_catalogue();
        let counts = MenuCounts::of(&catalogue);
        let factions = &catalogue.skirmish.factions;
        assert!(counts.random && counts.factions == 9);
        let initial = MenuSelection {
            enemies: 2,
            ..MenuSelection::initial(&catalogue)
        };
        // The human player's row: the nine countries with their side, then RANDOM, then the
        // first again.
        let mut selection = initial;
        let mut texts = Vec::new();
        for entry in 0..=counts.factions {
            texts.push(row_text(Row::Country(0), selection, &catalogue));
            let options = selection.options();
            if entry < counts.factions {
                assert_eq!(selection.players()[0].faction, entry);
                assert!(!options.random_countries.contains(&HUMAN));
                let side = factions[entry].side.expect("a country");
                assert_eq!(
                    texts[entry],
                    format!(
                        "{} ({})",
                        factions[entry].label,
                        if side == sim::Side::Allied {
                            "Allied"
                        } else {
                            "Soviet"
                        }
                    )
                );
            } else {
                assert_eq!(texts[entry], "RANDOM");
                assert_eq!(
                    options.random_countries,
                    [HUMAN, PlayerId(1), PlayerId(2), PlayerId(3)]
                );
            }
            assert!(selection.valid(counts));
            selection = stepped(selection, Row::Country(0), true, counts);
        }
        assert_eq!(selection, initial, "wraps");
        assert_eq!(
            texts,
            [
                "America (Allied)",
                "Korea (Allied)",
                "France (Allied)",
                "Germany (Allied)",
                "Great Britain (Allied)",
                "Libya (Soviet)",
                "Iraq (Soviet)",
                "Cuba (Soviet)",
                "Russia (Soviet)",
                "RANDOM"
            ]
        );
        // Every computer player has a row of its own: RANDOM at first, and a fixed country
        // takes it out of the players the simulation draws for.
        assert_eq!(
            initial.options().random_countries,
            [PlayerId(1), PlayerId(2), PlayerId(3)]
        );
        let cuba = factions
            .iter()
            .position(|faction| faction.id == "cuba")
            .expect("cuba");
        let mut second = initial;
        while second.countries[2] != Some(cuba) {
            second = stepped(second, Row::Country(2), false, counts);
        }
        assert_eq!(
            second.options().random_countries,
            [PlayerId(1), PlayerId(3)]
        );
        assert_eq!(second.players()[2].faction, cuba);
        assert_eq!(
            row_text(Row::Country(2), second, &catalogue),
            "Cuba (Soviet)"
        );
        assert_eq!(row_text(Row::Country(1), second, &catalogue), "RANDOM");
        assert_eq!(
            MenuSelection {
                countries: second.countries,
                ..initial
            },
            second,
            "the row of one player changes nothing else"
        );
        // A row of a slot beyond the match's players is not part of the match.
        let beyond = stepped(second, Row::Country(7), true, counts);
        assert_eq!(beyond.options(), second.options());
        assert_eq!(beyond.players(), second.players());
        // Difficulties: Easy, Medium, Brutal for each computer player on its own.
        let names: Vec<String> = (0..counts.difficulties)
            .map(|difficulty| difficulty_text(difficulty, &catalogue))
            .collect();
        assert_eq!(names, ["Easy", "Medium", "Brutal"]);
        let brutal = stepped(initial, Row::Difficulty(2), true, counts);
        let easy = stepped(initial, Row::Difficulty(2), false, counts);
        assert_eq!(row_text(Row::Difficulty(2), brutal, &catalogue), "Brutal");
        assert_eq!(row_text(Row::Difficulty(2), easy, &catalogue), "Easy");
        assert_eq!(
            stepped(brutal, Row::Difficulty(2), true, counts),
            easy,
            "wraps"
        );
        let difficulties = |selection: MenuSelection| -> Vec<Controller> {
            selection
                .players()
                .iter()
                .map(|setup| setup.controller)
                .collect()
        };
        assert_eq!(
            difficulties(brutal),
            [
                Controller::Human,
                Controller::Ai { difficulty: 1 },
                Controller::Ai { difficulty: 2 },
                Controller::Ai { difficulty: 1 },
            ]
        );
        assert_eq!(brutal.options(), initial.options());
        // In the match every computer player plays at its own difficulty.
        let mixed = stepped(brutal, Row::Difficulty(1), false, counts);
        let (host, ..) = set_up(&catalogue, mixed.with_countries(0, cuba, 9), 0);
        let ai = host.ai().expect("computer players");
        let think = |player: u8| {
            ai.controller(PlayerId(player))
                .expect("controller")
                .difficulty()
                .id
                .clone()
        };
        assert_eq!([think(1), think(2), think(3)], ["easy", "brutal", "medium"]);
        // A selection that names a faction, a RANDOM country or a difficulty the catalogue does
        // not have is not valid.
        let mut outside = initial;
        outside.countries[1] = Some(counts.factions);
        assert!(!outside.valid(counts));
        let mut outside = initial;
        outside.difficulties[3] = counts.difficulties;
        assert!(!outside.valid(counts));
        // (A slot beyond the players is not looked at.)
        let mut beyond = initial;
        beyond.difficulties[4] = counts.difficulties;
        assert!(beyond.valid(counts));
        assert!(
            !initial.valid(MenuCounts {
                random: false,
                ..counts
            }),
            "RANDOM needs countries to draw from"
        );
    }

    #[test]
    fn random_countries_are_drawn_by_the_simulation_and_named_once_the_match_starts() {
        let catalogue = production_catalogue();
        let factions = &catalogue.skirmish.factions;
        let demo = catalogue
            .maps
            .iter()
            .position(|map| map.id == "demo")
            .expect("demo");
        // Everyone RANDOM but the second computer player, who plays France.
        let france = factions
            .iter()
            .position(|faction| faction.id == "france")
            .expect("france");
        let mut selection = MenuSelection {
            map: demo,
            enemies: 4,
            color: 2,
            ..MenuSelection::initial(&catalogue)
        };
        selection.countries[0] = None;
        selection.countries[2] = Some(france);
        assert_eq!(selection.players().len(), 6);
        let mut drawn = BTreeSet::new();
        for seed in 0..8 {
            let (host, start, played) = set_up(&catalogue, selection, seed);
            assert_eq!(played.0.len(), 6);
            for (slot, entry) in played.0.iter().enumerate() {
                assert_eq!(entry.player, PlayerId(slot as u8));
                // Whatever was chosen, the player plays a country of the catalogue: the one its
                // state in the simulation names.
                let faction = &factions[entry.faction];
                assert!(entry.side.is_some() && entry.side == faction.side);
                assert_eq!(
                    host.players()
                        .and_then(|players| players.get(entry.player))
                        .and_then(sim::PlayerState::country),
                    faction.country
                );
                assert_eq!(entry.label, faction.label);
                assert_eq!(entry.country, country_text(Some(entry.faction), &catalogue));
                assert_ne!(entry.country, "RANDOM");
                assert_eq!(played.label(entry.player), Some(faction.label.as_str()));
                assert_eq!(entry.difficulty.is_none(), slot == 0);
                if slot != 2 {
                    drawn.insert((slot, entry.faction));
                }
            }
            assert_eq!(played.0[2].faction, france, "a chosen country is played");
            assert_eq!(played.label(PlayerId(6)), None);
            // What the match starts with follows from the countries played.
            let dialog = catalogue.definitions.rules().expect("rules").dialog;
            assert_eq!(
                (start.units, start.buildings),
                starting_totals(
                    &catalogue,
                    played.factions(),
                    dialog.unit_count as u32,
                    true
                )
            );
            assert_eq!(
                selection_totals(&catalogue, selection),
                None,
                "not known before"
            );
            // The same seed draws the same countries: a match can be replayed.
            assert_eq!(set_up(&catalogue, selection, seed).2, played);
            // The lines of the list shown while the match is paused or over, in the colours of
            // the match (the human player chose Gold).
            let own = &played.0[0];
            assert_eq!(
                played_line(0, &played, selection.color, false),
                Some(format!("You - Gold - {}", own.country))
            );
            assert_eq!(
                played_line(2, &played, selection.color, false).as_deref(),
                Some("AI 2 - Purple - France (Allied), Medium")
            );
            assert_eq!(
                played_line(2, &played, selection.color, true).as_deref(),
                Some("AI 2 - Purple - France (Allied), Medium, defeated")
            );
            assert_eq!(played_line(6, &played, selection.color, false), None);
            assert_eq!(
                start_text(&own.country),
                format!("You play {}", own.country)
            );
        }
        // Over the seeds the draw varies: several countries, of both sides, for the human
        // player as for the computer players.
        let countries: BTreeSet<usize> = drawn.iter().map(|&(_, faction)| faction).collect();
        assert!(countries.len() >= 6, "{countries:?}");
        let human: BTreeSet<usize> = drawn
            .iter()
            .filter(|&&(slot, _)| slot == 0)
            .map(|&(_, faction)| faction)
            .collect();
        assert!(human.len() >= 3, "{human:?}");
        for side in sim::Side::ALL {
            assert!(
                countries
                    .iter()
                    .any(|&faction| factions[faction].side == Some(side))
            );
        }
        // The names the notices and the hover label use.
        assert_eq!(player_name("Blue", Some("France")), "Blue Player (France)");
        assert_eq!(player_name("Blue", None), "Blue Player");
        assert_eq!(MatchPlayers::default().label(HUMAN), None);
    }

    #[test]
    fn map_growth_is_an_option_and_off_limits_the_enemies_to_the_starts_of_the_file() {
        // docs/DECISIONS.md R4: MAP GROWTH is off at first. The map is then played as its file
        // defines it, with at most a player on every start position the file lists; on, it
        // grows with the players, who may be ten on every map.
        let catalogue = production_catalogue();
        let counts = MenuCounts::of(&catalogue);
        let initial = MenuSelection::versus(&catalogue);
        assert!(!initial.map_growth && !MenuSelection::default().map_growth);
        for (map, def) in catalogue.skirmish.maps.iter().enumerate() {
            assert_eq!(counts.starts[map], def.starts.len());
            assert_eq!(
                counts.enemies(map, false),
                def.starts.len() - 1,
                "{}",
                def.id
            );
            assert!((3..=9).contains(&counts.enemies(map, false)), "{}", def.id);
            assert_eq!(counts.enemies(map, true), sim::MAX_MATCH_PLAYERS - 1);
        }
        assert_eq!(counts.enemies(counts.maps, false), 0, "no such map");
        let riverlands = catalogue
            .maps
            .iter()
            .position(|map| map.id == "riverlands")
            .expect("riverlands");
        let filed = &catalogue.maps[riverlands].map;
        assert_eq!(catalogue.skirmish.maps[riverlands].starts.len(), 4);
        let mut selection = MenuSelection {
            map: riverlands,
            ..initial
        };
        // Off: 1 to 3 enemies, on the map of the file whatever their number.
        let mut seen = Vec::new();
        for _ in 0..4 {
            seen.push(selection.enemy_count());
            assert_eq!(
                row_text(Row::Enemies, selection, &catalogue),
                format!("{} of 3", selection.enemy_count())
            );
            assert_eq!(
                row_text(Row::Map, selection, &catalogue),
                format!("riverlands  {} x {}", filed.view.width, filed.view.height)
            );
            let (plan, played) = match_plan(&catalogue, selection, 0).expect("a plan");
            assert!(played.as_filed);
            assert_eq!((&plan.grid, &plan.looks), (&filed.grid, &filed.looks));
            assert_eq!(plan.skirmish, catalogue.skirmish, "the starts of the file");
            let start = crate::sim_host::check_match(&plan).expect("the match sets up");
            assert_eq!(
                Some((start.units, start.buildings)),
                selection_totals(&catalogue, selection)
            );
            selection = stepped(selection, Row::Enemies, true, counts);
        }
        assert_eq!(seen, [1, 2, 3, 1]);
        // One more than the file has starts for is not offered.
        let too_many = MenuSelection {
            enemies: 3,
            ..selection
        };
        assert!(!too_many.valid(counts));
        assert_eq!(
            menu_effect(MenuButton::Start, too_many, counts),
            MenuEffect::Nothing
        );
        // On: up to nine, on a map an eighth larger for every player beyond the second.
        let selection = MenuSelection {
            enemies: 0,
            ..selection
        };
        let grown = stepped(selection, Row::MapGrowth, true, counts);
        assert!(grown.map_growth);
        assert_eq!(row_text(Row::MapGrowth, grown, &catalogue), "ON");
        assert_eq!(row_text(Row::Enemies, grown, &catalogue), "1 of 9");
        assert_eq!(
            grown.options(),
            selection.options(),
            "not an option of the simulation"
        );
        let nine = stepped(grown, Row::Enemies, false, counts);
        assert_eq!(nine.enemy_count(), 9);
        assert!(nine.valid(counts) && too_many.enemies < nine.enemies);
        assert_eq!(
            row_text(Row::Map, nine, &catalogue),
            format!(
                "riverlands  {} x {}",
                2 * filed.view.width,
                2 * filed.view.height
            )
        );
        let (plan, played) = match_plan(&catalogue, nine, 0).expect("a plan");
        assert!(!played.as_filed);
        assert_eq!(plan.grid.width(), 2 * filed.view.width);
        assert_eq!(plan.skirmish.maps[riverlands].starts.len(), 10);
        crate::sim_host::check_match(&plan).expect("the match sets up");
        // Switching it off again keeps as many enemies as the file has starts for, and so does
        // moving on to a map with fewer.
        let off = stepped(nine, Row::MapGrowth, true, counts);
        assert!(!off.map_growth);
        assert_eq!(off.enemy_count(), 3);
        assert!(off.valid(counts));
        assert_eq!(row_text(Row::Enemies, off, &catalogue), "3 of 3");
        let demo = catalogue
            .maps
            .iter()
            .position(|map| map.id == "demo")
            .expect("demo");
        let mut next = MenuSelection {
            map: demo,
            enemies: counts.enemies(demo, false) - 1,
            ..initial
        };
        assert_eq!(next.enemy_count(), 5);
        while next.map != riverlands {
            next = stepped(next, Row::Map, true, counts);
            assert!(next.valid(counts), "{next:?}");
        }
        assert_eq!(next.enemy_count(), 3);
    }

    #[test]
    fn the_menu_and_its_longest_texts_fit_the_smallest_window() {
        // The measures of the menu add up to no more than the smallest window (the smoke test
        // measures the menu as it is laid out against the same numbers).
        let size = menu_size();
        assert!(size.x <= MENU_FIT.x && size.y <= MENU_FIT.y, "{size}");
        assert!(size.x > 1000.0 && size.y > 400.0, "{size}");
        assert_eq!(Row::MATCH.len() + Row::RULES.len(), 19);
        let rows: BTreeSet<Row> = Row::MATCH.into_iter().chain(Row::RULES).collect();
        assert_eq!(rows.len(), 19, "every row once");
        // Every text a row can show fits its place. The interface font is FiraMono, in which
        // every character is 0.6 of the font size wide.
        let width = |text: &str, font: f32| text.chars().count() as f32 * 0.6 * font;
        let catalogue = production_catalogue();
        let counts = MenuCounts::of(&catalogue);
        let initial = MenuSelection::initial(&catalogue);
        for (rows, (label, value)) in [
            (Row::MATCH.as_slice(), MATCH_WIDTHS),
            (Row::RULES.as_slice(), RULES_WIDTHS),
        ] {
            for &row in rows {
                assert!(width(row.label(), ROW_FONT) <= label, "{row:?}");
                // Every entry of the row, with MAP GROWTH on and off and the most players
                // (which is when the map row shows the largest sizes), on every map.
                for map in 0..counts.maps {
                    for map_growth in [false, true] {
                        let mut selection = MenuSelection {
                            map,
                            map_growth,
                            enemies: counts.enemies(map, map_growth) - 1,
                            ..initial
                        };
                        let (_, entries) = selection.position(row, counts);
                        for _ in 0..entries {
                            let text = row_text(row, selection, &catalogue);
                            assert!(width(&text, ROW_FONT) <= value, "{row:?}: {text:?}");
                            selection = stepped(selection, row, true, counts);
                        }
                    }
                }
            }
        }
        let full = MenuSelection {
            enemies: sim::MAX_MATCH_PLAYERS - 2,
            map_growth: true,
            ..initial
        };
        for slot in 0..sim::MAX_MATCH_PLAYERS {
            for (row, place) in [
                (Row::Country(slot), COUNTRY_WIDTH),
                (Row::Difficulty(slot), DIFFICULTY_WIDTH),
            ] {
                let (_, entries) = full.position(row, counts);
                assert_eq!(entries == 0, row == Row::Difficulty(0));
                let mut selection = full;
                for _ in 0..entries {
                    let text = row_text(row, selection, &catalogue);
                    assert!(width(&text, PLAYER_FONT) <= place, "{row:?}: {text:?}");
                    selection = stepped(selection, row, true, counts);
                }
            }
            for color in 0..crate::presentation::TEAM_PALETTE.len() {
                let tag = player_tag(slot, MenuSelection { color, ..full }).expect("a player");
                assert!(width(&tag, PLAYER_FONT) <= TAG_WIDTH, "{tag:?}");
            }
        }
        assert_eq!(player_tag(sim::MAX_MATCH_PLAYERS, full), None);
    }

    #[test]
    fn the_end_screen_says_what_decided_the_match_by_its_rule() {
        // RA1 item 3 (docs/DECISIONS.md R13): in a short game a player is defeated without
        // buildings and base unit, in a long game without buildings and units.
        let won = Some(MatchOutcome::Won {
            winner: HUMAN,
            tick: 9,
        });
        let lost = Some(MatchOutcome::Won {
            winner: OPPONENT,
            tick: 9,
        });
        let draw = Some(MatchOutcome::Draw { tick: 9 });
        for short_game in [true, false] {
            assert_eq!(
                end_reason(None, false, short_game),
                None,
                "the match goes on"
            );
        }
        assert_eq!(
            end_reason(won, false, true),
            Some("Short game: no enemy has a building or an MCV left")
        );
        assert_eq!(
            end_reason(won, false, false),
            Some("Long game: no enemy has a building or a unit left")
        );
        // The human player's defeat ends the match for the human, whether or not the computer
        // players go on.
        for outcome in [lost, None] {
            assert_eq!(
                end_reason(outcome, true, true),
                Some("Short game: you have no building and no MCV left")
            );
            assert_eq!(
                end_reason(outcome, true, false),
                Some("Long game: you have no building and no unit left")
            );
        }
        assert_eq!(
            end_reason(draw, true, true),
            Some("Short game: the last players lost their buildings and MCVs together")
        );
        assert_eq!(
            end_reason(draw, true, false),
            Some("Long game: the last players lost their buildings and units together")
        );
        // There is a reason exactly when there is a headline.
        for outcome in [None, won, lost, draw] {
            for defeated in [false, true] {
                assert_eq!(
                    end_text(outcome, defeated).is_some(),
                    end_reason(outcome, defeated, true).is_some()
                );
            }
        }
    }

    #[test]
    fn the_catalogue_rejects_starting_credits_the_lobby_does_not_offer() {
        // Debt of RA1-B (docs/DECISIONS.md R9 item 8): the starting credits of the skirmish
        // file were not checked against the lobby of the rules file. A match has them while
        // the credits row is left alone, so they must be an amount the row offers.
        let catalogue = production_catalogue();
        let root = assets();
        let data = root.join(crate::units::DATA_DIR);
        let lobby = Lobby::of(&catalogue);
        assert!(lobby.offers_credits(catalogue.skirmish.starting_credits));
        let (low, high, step) = (lobby.min_credits, lobby.max_credits, lobby.credits_step);
        assert!(lobby.offers_credits(low) && lobby.offers_credits(high));
        assert!(lobby.offers_credits(low + step) && lobby.offers_credits(high - step));
        for credits in [low - step, low - 1, low + 1, high + 1, high + step, 0] {
            assert!(!lobby.offers_credits(credits), "{credits}");
            let mut outside = catalogue.clone();
            outside.skirmish.starting_credits = credits;
            let error = check_catalogue(&outside, &data).expect_err("outside the lobby");
            for part in [
                format!("data file {}", root.join(SKIRMISH_PATH).display()),
                format!("starting_credits: {credits} is not an amount"),
                format!("({low} to {high} in steps of {step})"),
            ] {
                assert!(error.contains(&part), "{error:?} lacks {part:?}");
            }
        }
        // The lowest amount of the lobby is accepted (the shipped file has the highest).
        let mut lowest = catalogue.clone();
        lowest.skirmish.starting_credits = low;
        check_catalogue(&lowest, &data).unwrap_or_else(|error| panic!("{error}"));
        // A ruleset without a rules file has no lobby: its credits are the only amount.
        let legacy = legacy_catalogue();
        let lobby = Lobby::of(&legacy);
        assert_eq!(
            (lobby.credit_amounts(), lobby.unit_counts()),
            (1, 1),
            "{lobby:?}"
        );
        assert!(lobby.offers_credits(legacy.skirmish.starting_credits));
        let selection = MenuSelection::initial(&legacy);
        assert!(selection.valid(MenuCounts::of(&legacy)));
        assert!(
            selection.options().random_countries.is_empty(),
            "nothing to draw from"
        );
    }
}
