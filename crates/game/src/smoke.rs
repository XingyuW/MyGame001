//! `--smoke-test <frames>`: run the real application for a fixed number of frames, then exit.
//!
//! Exits with code 0 only if no asset failed to load, the map was validated and spawned, all
//! configured starting units and buildings were spawned, and the commands that the smoke test
//! issues through `CommandQueue` were accepted and took effect: the move, which must have moved a
//! unit if at least [`MOVE_CHECK_TICKS`] ticks ran after it, and (M3) the production of the first
//! building the player can queue, which must have been charged for if at least
//! [`PRODUCTION_CHECK_TICKS`] ticks ran after it. From M4, once a tick has run the human player
//! must see some cells (the fog is computed), and every unit and building view must be shown
//! exactly when the human player sees what it draws (unseen enemies are hidden).
//!
//! From M5 the run loads the full interface, and its orders go the interface's way: the move is
//! built by `crate::selection::move_command` and the production by the sidebar's
//! `crate::sidebar::button_effect`, and both are pushed into `crate::sim_host::CommandSink`. It
//! also requires a sidebar button for every production item, the sidebar's credits to show the
//! player's credits, the minimap to have been drawn, every sound and the music to exist, and,
//! in runs of at least [`PAUSE_MIN_FRAMES`] frames, that the game pauses for [`PAUSE_FRAMES`]
//! frames without a tick and refuses a command meanwhile.
//!
//! From M6 the game starts in the menu, and the run plays the game flow the menu's and the end
//! screen's way, with their pure functions (`crate::skirmish::menu_effect`, `end_effect`): it
//! requires the menu with all of its buttons, starts the match from it, and, in runs of at least
//! [`FLOW_MIN_FRAMES`] frames, restarts the match after [`FLOW_TICKS`] ticks, then returns to the
//! menu after [`FLOW_TICKS`] more and starts again. The checks above apply to the last match, which
//! has the starting units and buildings of the skirmish file (no more, no fewer views than the
//! simulation has units and buildings: nothing of the earlier matches is left), and its computer
//! player must have issued commands once [`AI_CHECK_TICKS`] ticks have run. No command of the
//! computer player may be rejected in any match of the run (M6 review 1 P2-4): the smoke matches
//! are too short for contact between the players, so no loss can explain a rejection (D43
//! item 5), and a rejected command means the computer player issued an unusable one.
//!
//! From M7 the last match of a long run is started with the human player's country row on the
//! second faction; no sidebar button of an item outside the roster of the
//! human player's faction may be displayed; the menu has its Options button, and the options
//! screen all of its buttons; and the run changes two options the options screen's way
//! (`crate::options::options_effect`), which must be written to the run's own settings file
//! ([`settings_path`], removed at the end) and read back unchanged (docs/DECISIONS.md D52).
//!
//! Since the multi-enemy follow-up, a long run's last match has [`SMOKE_ENEMIES`] computer
//! players, chosen with the menu's enemy row; each must be a player of its own (own economy, AI
//! and colour), "unlock all" (chosen for the first two matches only) and Super Me apply to the
//! human player alone, and every computer player's rejections count. The last match runs at the
//! fastest game speed, which must set the fixed timestep, and must still pause.
//!
//! Since RA1 a match starts as Red Alert 2's lobby starts it: every player with the base unit of
//! its side (an MCV) and its starting units, and no building (`crate::skirmish::starting_totals`).
//! The run deploys the human player's base unit after the first tick, as the D key does, moves
//! the other units, and queues the first building once the Construction Yard stands, which it
//! must within [`MOVE_CHECK_TICKS`] ticks of the order. The computer players deploy theirs
//! themselves.
//!
//! With the lobby in the menu (RA1-C) a long run plays four matches, each chosen by the menu's
//! buttons, and compares every one with its choices once it has run [`FLOW_TICKS`] ticks
//! ([`match_problems`]: players, countries, credits, starting units and buildings, rules, teams,
//! colours, the human player's extensions, the size of the map):
//!
//! 1. In the menu the run first shows the fullest menu (ten players, the longest text in every
//!    place) and measures it as it is laid out ([`MenuLayout`]): every button is there, no two
//!    overlap, every text fits its place, and the menu covers no more than its measures say
//!    (`crate::skirmish::menu_size`) and than a window of 1280 x 720 holds.
//! 2. The first match has every row of the lobby away from its default ([`choose_first`]): a
//!    Soviet country, the lowest credits, unit count 0, the long game, free for all,
//!    superweapons and crates off, MAP GROWTH on with more players than the map file has start
//!    positions, a fixed country and another difficulty for two computer players, Super Me and
//!    ALL UNLOCKED. In it the run plays the MCV drill ([`Drill`]): it deploys the MCV, packs the
//!    Construction Yard up again with the game's Repack control, and deploys it once more,
//!    checking the simulation and the cards of the sidebar after each step.
//! 3. The match is restarted, and must hold the same choices.
//! 4. The third match starts with the Construction Yards and has MCV repacks off
//!    ([`choose_yards`]): the yard offers no repack, and the cards are those of the player's side.
//! 5. The last match has the lobby's defaults again ([`choose_last`]), with RANDOM countries for
//!    the computer players; the checks of M1 to M7 apply to it. Its cards must be among those
//!    the sidebar's rule lists (`crate::sidebar::listed_items`).

use std::collections::BTreeMap;

use bevy::asset::UntypedAssetLoadFailedEvent;
use bevy::prelude::*;
use sim::{
    Building, BuildingKind, CellPos, Command, Definitions, Mobile, Owner, PlayerState, Players,
    Position, ProductionItem, SimEvent, SimId, SubPos,
};

use crate::audio::{MusicPlayer, Sfx, SoundBank, VolumeSettings};
use crate::buildings::{BuildingStatus, BuildingView, building_shown};
use crate::construction::{InterfaceMode, production_menu};
use crate::map::MapStatus;
use crate::minimap::MinimapState;
use crate::options::{
    CameraOptions, OPTIONS_BUTTONS, OptionRow, OptionsButton, OptionsEffect, Settings,
    SettingsFile, options_effect, read_settings,
};
use crate::sidebar::{
    SidebarButton, SidebarEffect, SidebarText, button_effect, credits_text, item_status,
};
use crate::sim_host::{CommandRefused, CommandSink, SimHost};
use crate::skirmish::{
    Catalogue, EndButton, EndEffect, LastMatch, MENU_FIT, MatchPlayers, MenuBlock, MenuButton,
    MenuCounts, MenuEffect, MenuRoot, MenuSelection, Row, Screen, ScreenText, begin_match,
    end_effect, end_match, menu_effect, menu_size, starting_totals,
};
use crate::units::{HUMAN, UnitStatus, UnitView, unit_shown};

/// The cell of the demo map that the smoke test orders the player's units to (grass, across the
/// open ground from their starting area).
pub const MOVE_TARGET: CellPos = CellPos::new(20, 26);

/// Ticks after the order from which some unit must have moved.
pub const MOVE_CHECK_TICKS: u64 = 40;

/// Ticks after the production order from which credits must have been charged for it.
pub const PRODUCTION_CHECK_TICKS: u64 = 2;

/// Runs of at least this many frames pause the game in the middle (M5).
pub const PAUSE_MIN_FRAMES: u32 = 300;

/// Frames the pause lasts.
pub const PAUSE_FRAMES: u32 = 30;

/// Runs of at least this many frames restart the match, return to the menu, and start again
/// before the checks (M6).
pub const FLOW_MIN_FRAMES: u32 = 300;

/// Ticks a match of the game flow runs before the next step.
pub const FLOW_TICKS: u64 = 10;

/// Ticks of the last match after which its computer player must have issued commands (its first
/// turn comes within its think interval, at most `sim::MAX_THINK_INTERVAL`).
pub const AI_CHECK_TICKS: u64 = 60;

/// The seed of the smoke test's matches.
const SMOKE_SEED: u64 = 6;

/// Computer players of a long run's matches.
pub const SMOKE_ENEMIES: usize = 3;

/// The settings file of a smoke run: a file of its own in the temporary directory (removed at the
/// end of the run), so that a smoke run never reads or changes the player's settings (M7, D52).
pub fn settings_path() -> std::path::PathBuf {
    std::env::temp_dir()
        .join(format!("mygame001-smoke-{}", std::process::id()))
        .join("settings.txt")
}

pub struct SmokeTestPlugin {
    pub frames: u32,
}

impl Plugin for SmokeTestPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SmokeTest::new(self.frames))
            .add_systems(
                Update,
                (
                    measure_menu,
                    play_flow,
                    play_drill,
                    change_options,
                    order_move,
                    order_production,
                    watch_events,
                    check_pause,
                )
                    .chain(),
            )
            .add_systems(
                Last,
                (fail_on_asset_load_error, count_frames, fail_on_early_exit)
                    .chain()
                    // Bevy's exit when no window is left (`exit_on_all_closed`) runs in `Last`, in
                    // `ExitSystems`; the early-exit check must see its message.
                    .after(bevy::window::ExitSystems),
            );
    }
}

#[derive(Resource, Debug)]
struct SmokeTest {
    frames: u32,
    rendered: u32,
    /// The tick at which the move was ordered, and the units' positions then.
    order: Option<(u64, BTreeMap<SimId, SubPos>)>,
    /// The tick at which production was ordered, and the item (`None` if the player could
    /// queue nothing).
    production: Option<(u64, Option<ProductionItem>)>,
    rejected: bool,
    /// Commands of the computer player rejected in any match of the run (M6 review 1 P2-4).
    ai_rejected: u64,
    pause: PauseCheck,
    flow: Flow,
    /// Whether the run has changed its options (M7).
    options_changed: bool,
    /// Menu buttons found when the menu was first shown (`None` before).
    menu_buttons: Option<usize>,
    /// Matches started.
    starts: u32,
    /// Frames the flow has stood in the menu it is in (RA1: the layout check waits for them).
    menu_frames: u32,
    /// The menu's choices, kept while the run shows the fullest menu for the layout check.
    saved: Option<MenuSelection>,
    /// The menu as it was last laid out ([`measure_menu`]).
    layout: Option<MenuLayout>,
    /// The problems of the menu's layout, once it has been checked (`None` before; a run too
    /// short for the game flow does not check it).
    layout_problems: Option<usize>,
    /// Checks of the game flow that failed (RA1): a match that does not hold what its menu
    /// choices say, or a step of the MCV drill that did not happen. Each is logged.
    flow_problems: usize,
    /// Whether the running match has been compared with its menu choices.
    compared: bool,
    /// The MCV drill of the first match.
    drill: Drill,
}

impl SmokeTest {
    fn new(frames: u32) -> Self {
        Self {
            frames,
            rendered: 0,
            order: None,
            production: None,
            rejected: false,
            ai_rejected: 0,
            pause: PauseCheck::default(),
            flow: Flow::default(),
            menu_buttons: None,
            starts: 0,
            options_changed: false,
            menu_frames: 0,
            saved: None,
            layout: None,
            layout_problems: None,
            flow_problems: 0,
            compared: false,
            drill: Drill::default(),
        }
    }

    /// Records a failed check of the game flow.
    fn problem(&mut self, what: impl std::fmt::Display) {
        error!("smoke test failed: {what}");
        self.flow_problems += 1;
    }
}

/// The game flow the smoke test plays (M6; RA1: with the lobby's options).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Flow {
    /// In the menu; its layout is checked, the options of the first match are chosen, and the
    /// first match starts.
    #[default]
    Menu,
    /// The first match runs, with the options of [`OPTION_PRESSES`]; the MCV drill is played in
    /// it, and it is restarted after [`FLOW_TICKS`] ticks.
    First,
    /// The restarted match runs; the game returns to the menu after [`FLOW_TICKS`] ticks.
    Restarted,
    /// Back in the menu; the match that starts with the Construction Yards is chosen and
    /// starts.
    MenuAgain,
    /// The match with the yards runs; the game returns to the menu after [`FLOW_TICKS`] ticks.
    Yards,
    /// In the menu for the last time; the lobby's defaults are restored and the last match
    /// starts.
    MenuLast,
    /// The last match runs, and the checks of M1 to M5 apply to it.
    Last,
}

/// The MCV drill of the first match (RA1): the run deploys the human player's base unit, packs
/// the Construction Yard up again with the game's Repack control, and deploys it once more,
/// looking at the simulation after each step.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Drill {
    /// Not begun: the player owns its base unit and no building, and no card is listed.
    #[default]
    Start,
    /// Deploy was ordered; the yard must stand.
    Deployed,
    /// Repack was ordered; the base unit must be back.
    Repacked,
    /// Deploy was ordered again; the yard must stand again.
    Redeployed,
    Done,
}

/// Ticks of the first match within which its MCV drill must be done.
pub const DRILL_TICKS: u64 = 60;

/// Frames the flow stands in a menu before its layout is measured: the texts change in the
/// frame after the choices, and the layout follows them.
pub const LAYOUT_FRAMES: u32 = 4;

/// The map of the first two matches of a long run: the one whose file lists the fewest start
/// positions, so that with MAP GROWTH on a match has more players than the file has starts.
fn growth_map(catalogue: &Catalogue) -> usize {
    (0..catalogue.skirmish.maps.len())
        .min_by_key(|&map| catalogue.skirmish.maps[map].starts.len())
        .unwrap_or(0)
}

/// The menu with the longest text in every place, for the layout check: the longest map name
/// at its largest size, ten players who all play the country with the longest name at the
/// difficulty with the longest, and the longest entry of every other row.
fn fullest_menu(catalogue: &Catalogue, selection: MenuSelection) -> MenuSelection {
    let longest = |count: usize, text: &dyn Fn(usize) -> String| {
        (0..count)
            .max_by_key(|&index| text(index).len())
            .unwrap_or(0)
    };
    let country = longest(catalogue.skirmish.factions.len(), &|index| {
        crate::skirmish::country_text(Some(index), catalogue)
    });
    let difficulty = longest(catalogue.ai.difficulties.len(), &|index| {
        crate::skirmish::difficulty_text(index, catalogue)
    });
    let lobby = MenuCounts::of(catalogue).lobby;
    MenuSelection {
        map: longest(catalogue.maps.len(), &|index| {
            catalogue.maps[index].id.clone()
        }),
        enemies: sim::MAX_MATCH_PLAYERS - 2,
        map_growth: true,
        free_for_all: true,
        color: longest(crate::presentation::TEAM_PALETTE.len(), &|index| {
            crate::presentation::TEAM_PALETTE[index].0.to_owned()
        }),
        countries: [Some(country); sim::MAX_MATCH_PLAYERS],
        difficulties: [difficulty; sim::MAX_MATCH_PLAYERS],
        credits: Some(lobby.max_credits),
        unit_count: Some(lobby.max_unit_count),
        super_me: false,
        unit_speed: crate::skirmish::UNIT_SPEEDS.len() - 1,
        unlock_all: true,
        ..selection
    }
}

/// A node as it is laid out, in logical pixels; `None` for one that is not displayed.
fn node_rect(node: &ComputedNode, transform: &UiGlobalTransform) -> Option<Rect> {
    let scale = node.inverse_scale_factor();
    let size = node.size() * scale;
    (size != Vec2::ZERO).then(|| Rect::from_center_size(transform.translation * scale, size))
}

/// The menu as it is laid out: its blocks, its buttons, and its texts that change, each with
/// the place it stands in.
#[derive(Debug, Clone, Default, PartialEq)]
struct MenuLayout {
    window: Vec2,
    blocks: Vec<Rect>,
    buttons: Vec<Rect>,
    texts: Vec<(Rect, Rect)>,
}

impl MenuLayout {
    /// The ground the menu covers.
    fn extent(&self) -> Option<Rect> {
        self.blocks
            .iter()
            .chain(&self.buttons)
            .chain(self.texts.iter().map(|(text, _)| text))
            .copied()
            .reduce(|all, rect| all.union(rect))
    }

    /// What is wrong with the layout; empty if the menu shows all `buttons` buttons, covers no
    /// more ground than its measures say ([`menu_size`]) and than the smallest window holds
    /// ([`MENU_FIT`]), lies inside the window, and no two buttons overlap and every text fits
    /// its place.
    fn problems(&self, buttons: usize) -> Vec<String> {
        let Some(extent) = self.extent() else {
            return vec!["the menu is not laid out".to_owned()];
        };
        let mut problems = Vec::new();
        let (size, covered) = (menu_size(), extent.size());
        if covered.x > size.x + 1.0 || covered.y > size.y + 1.0 {
            problems.push(format!(
                "the menu covers {} x {} pixels, more than the {} x {} of its measures",
                covered.x, covered.y, size.x, size.y
            ));
        }
        if covered.x > MENU_FIT.x || covered.y > MENU_FIT.y {
            problems.push(format!(
                "the menu covers {} x {} pixels and does not fit a window of {} x {}",
                covered.x, covered.y, MENU_FIT.x, MENU_FIT.y
            ));
        }
        if extent.min.x < -0.5
            || extent.min.y < -0.5
            || extent.max.x > self.window.x + 0.5
            || extent.max.y > self.window.y + 0.5
        {
            problems.push(format!(
                "the menu reaches from ({}, {}) to ({}, {}), outside the window of {} x {}",
                extent.min.x,
                extent.min.y,
                extent.max.x,
                extent.max.y,
                self.window.x,
                self.window.y
            ));
        }
        if self.buttons.len() != buttons {
            problems.push(format!(
                "{} of {buttons} menu buttons are laid out",
                self.buttons.len()
            ));
        }
        let overlap = |a: &Rect, b: &Rect| {
            a.min.x < b.max.x - 0.5
                && b.min.x < a.max.x - 0.5
                && a.min.y < b.max.y - 0.5
                && b.min.y < a.max.y - 0.5
        };
        let overlapping = self
            .buttons
            .iter()
            .enumerate()
            .flat_map(|(index, a)| self.buttons[index + 1..].iter().map(move |b| (a, b)))
            .filter(|(a, b)| overlap(a, b))
            .count();
        if overlapping > 0 {
            problems.push(format!("{overlapping} pairs of menu buttons overlap"));
        }
        let outside = self
            .texts
            .iter()
            .filter(|(text, place)| {
                text.min.x < place.min.x - 0.5
                    || text.min.y < place.min.y - 0.5
                    || text.max.x > place.max.x + 0.5
                    || text.max.y > place.max.y + 0.5
            })
            .count();
        if outside > 0 {
            problems.push(format!("{outside} menu texts do not fit their place"));
        }
        problems
    }
}

/// The nodes of the menu, for [`measure_menu`].
type MenuNodes<'w, 's> = (
    Query<'w, 's, (&'static ComputedNode, &'static UiGlobalTransform), With<MenuBlock>>,
    Query<'w, 's, (&'static ComputedNode, &'static UiGlobalTransform), With<MenuButton>>,
    Query<
        'w,
        's,
        (
            &'static ComputedNode,
            &'static UiGlobalTransform,
            &'static ChildOf,
            &'static ScreenText,
        ),
    >,
    Query<'w, 's, (&'static ComputedNode, &'static UiGlobalTransform)>,
);

/// Measures the menu as the last frame laid it out, while it is shown.
fn measure_menu(
    mut smoke: ResMut<SmokeTest>,
    screen: Res<Screen>,
    window: Single<&Window, With<bevy::window::PrimaryWindow>>,
    nodes: MenuNodes,
) {
    if *screen != Screen::Menu || smoke.layout_problems.is_some() {
        return;
    }
    let (blocks, buttons, texts, places) = nodes;
    let rects = |nodes: &Query<(&ComputedNode, &UiGlobalTransform), _>| -> Vec<Rect> {
        nodes
            .iter()
            .filter_map(|(node, transform)| node_rect(node, transform))
            .collect()
    };
    smoke.layout = Some(MenuLayout {
        window: window.size(),
        blocks: blocks
            .iter()
            .filter_map(|(node, transform)| node_rect(node, transform))
            .collect(),
        buttons: rects(&buttons),
        texts: texts
            .iter()
            .filter(|(.., kind)| matches!(kind, ScreenText::Row(_) | ScreenText::Player(_)))
            .filter_map(|(node, transform, parent, _)| {
                let (place, at) = places.get(parent.parent()).ok()?;
                Some((node_rect(node, transform)?, node_rect(place, at)?))
            })
            .collect(),
    });
}

/// Presses the `>` (`forward`) or `<` button of `row` `times` times, as clicks on it do.
fn press(
    smoke: &mut SmokeTest,
    selection: &mut MenuSelection,
    counts: MenuCounts,
    row: Row,
    forward: bool,
    times: usize,
) {
    let button = if forward {
        MenuButton::Next(row)
    } else {
        MenuButton::Previous(row)
    };
    for _ in 0..times {
        match menu_effect(button, *selection, counts) {
            MenuEffect::Select(next) => *selection = next,
            effect => smoke.problem(format!("the menu button {button:?} did {effect:?}")),
        }
    }
}

/// Presses the `>` button of `row` until `reached` holds (at once if it already does); a
/// problem if no entry of the row makes it hold.
fn press_until(
    smoke: &mut SmokeTest,
    selection: &mut MenuSelection,
    counts: MenuCounts,
    row: Row,
    reached: impl Fn(&MenuSelection) -> bool,
) {
    let (_, entries) = selection.position(row, counts);
    for _ in 0..entries {
        if reached(selection) {
            return;
        }
        press(smoke, selection, counts, row, true, 1);
    }
    if !reached(selection) {
        smoke.problem(format!(
            "no entry of the menu row {row:?} is the one wanted"
        ));
    }
}

/// Computer players of the first two matches of a long run: one more than the map of those
/// matches lists start positions for beyond the human player's ([`growth_map`]; MAP GROWTH on
/// makes the missing one). Never more than a match may have.
fn grown_enemies(catalogue: &Catalogue) -> usize {
    catalogue.skirmish.maps[growth_map(catalogue)]
        .starts
        .len()
        .min(sim::MAX_MATCH_PLAYERS - 1)
}

/// Chooses the first match of a long run by the menu's buttons, changing the rows from the
/// lobby's defaults: the map with the fewest start positions with MAP GROWTH on and more
/// computer players than it lists starts for, FREE FOR ALL, the lowest credits (one step past
/// the highest wraps around), unit count 0 (likewise), the long game, superweapons and crates
/// off, a Soviet country for the human player, a fixed country for two computer players and
/// another difficulty for each, and the extensions Super Me (with the next unit speed and
/// colour) and ALL UNLOCKED. MCV start and MCV repacks stay on for the MCV drill.
fn choose_first(
    smoke: &mut SmokeTest,
    selection: &mut MenuSelection,
    catalogue: &Catalogue,
    counts: MenuCounts,
) {
    let map = growth_map(catalogue);
    press_until(smoke, selection, counts, Row::Map, |choice| {
        choice.map == map
    });
    press(smoke, selection, counts, Row::MapGrowth, true, 1);
    let enemies = grown_enemies(catalogue);
    press_until(smoke, selection, counts, Row::Enemies, |choice| {
        choice.enemy_count() == enemies
    });
    for row in [
        Row::TeamMode,
        Row::Credits,
        Row::UnitCount,
        Row::ShortGame,
        Row::Superweapons,
        Row::Crates,
        Row::SuperMe,
        Row::UnitSpeed,
        Row::Color,
        Row::UnlockAll,
    ] {
        press(smoke, selection, counts, row, true, 1);
    }
    let soviet = |choice: &MenuSelection| {
        choice.countries[0]
            .and_then(|faction| catalogue.skirmish.factions.get(faction))
            .is_some_and(|faction| faction.side == Some(sim::Side::Soviet))
    };
    press_until(smoke, selection, counts, Row::Country(0), soviet);
    // From RANDOM: the first country one way, the last the other.
    press(smoke, selection, counts, Row::Country(1), true, 1);
    press(smoke, selection, counts, Row::Country(2), false, 1);
    press(smoke, selection, counts, Row::Difficulty(1), true, 1);
    press(smoke, selection, counts, Row::Difficulty(2), false, 1);
    let lobby = counts.lobby;
    let chosen = selection.map_growth
        && selection.free_for_all
        && selection.credits == Some(lobby.min_credits)
        && selection.unit_count == Some(lobby.min_unit_count)
        && !selection.short_game
        && !selection.superweapons
        && !selection.crates
        && selection.mcv_start
        && selection.mcv_repacks
        && selection.super_me
        && selection.unlock_all
        && selection.players().len() > catalogue.skirmish.maps[map].starts.len()
        && selection.countries[1] == Some(0)
        && selection.countries[2] == Some(counts.factions.saturating_sub(1))
        && selection.difficulties[1] != selection.difficulties[2];
    if !chosen {
        smoke.problem(format!(
            "the menu's buttons did not choose the first match: {selection:?}"
        ));
    }
}

/// Chooses the match with the Construction Yards: MCV start and MCV repacks off, MAP GROWTH off
/// (which leaves as many computer players as the map file has start positions for), ALLIED AI
/// again, the standard tech tree again (so that the cards are those of the player's side), and
/// the credits and the unit count back at their highest by the `<` button, which wraps around
/// the other way.
fn choose_yards(
    smoke: &mut SmokeTest,
    selection: &mut MenuSelection,
    catalogue: &Catalogue,
    counts: MenuCounts,
) {
    for row in [
        Row::McvStart,
        Row::McvRepacks,
        Row::MapGrowth,
        Row::TeamMode,
        Row::UnlockAll,
    ] {
        press(smoke, selection, counts, row, true, 1);
    }
    press(smoke, selection, counts, Row::Credits, false, 1);
    press(smoke, selection, counts, Row::UnitCount, false, 1);
    let lobby = counts.lobby;
    let starts = catalogue.skirmish.maps[selection.map].starts.len();
    let chosen = !selection.mcv_start
        && !selection.mcv_repacks
        && !selection.map_growth
        && !selection.free_for_all
        && !selection.unlock_all
        && selection.players().len() == starts.min(sim::MAX_MATCH_PLAYERS)
        && selection.credits == Some(lobby.max_credits)
        && selection.unit_count == Some(lobby.max_unit_count);
    if !chosen {
        smoke.problem(format!(
            "the menu's buttons did not choose the match with the yards: {selection:?}"
        ));
    }
}

/// Chooses the last match: the lobby's defaults again (every option of the lobby back on, the
/// extensions off, the computer players' countries RANDOM and their difficulties as at first),
/// on the map the menu opened with (`opening`), with [`SMOKE_ENEMIES`] computer players, the
/// human player with the second country (M7: the checks cover a country other than the first),
/// and at the fastest game speed.
fn choose_last(
    smoke: &mut SmokeTest,
    selection: &mut MenuSelection,
    counts: MenuCounts,
    opening: MenuSelection,
) {
    for row in [
        Row::McvStart,
        Row::McvRepacks,
        Row::ShortGame,
        Row::Superweapons,
        Row::Crates,
        Row::SuperMe,
        Row::Color,
    ] {
        press(smoke, selection, counts, row, true, 1);
    }
    press_until(smoke, selection, counts, Row::Map, |choice| {
        choice.map == opening.map
    });
    press_until(smoke, selection, counts, Row::Enemies, |choice| {
        choice.enemy_count() == SMOKE_ENEMIES
    });
    let second = usize::from(counts.factions > 1);
    press_until(smoke, selection, counts, Row::Country(0), |choice| {
        choice.countries[0] == Some(second)
    });
    // Back to RANDOM the way each came, and to the difficulties of the opening menu.
    press(smoke, selection, counts, Row::Country(1), false, 1);
    press(smoke, selection, counts, Row::Country(2), true, 1);
    press(smoke, selection, counts, Row::Difficulty(1), false, 1);
    press(smoke, selection, counts, Row::Difficulty(2), true, 1);
    press_until(smoke, selection, counts, Row::GameSpeed, |choice| {
        choice.game_speed == crate::game_speed::SPEED_LEVELS.len() - 1
    });
    // Everything the simulation is set up from is the lobby's default again, but for the human
    // player's country.
    let mut expected = opening;
    expected.countries[0] = Some(second);
    expected.enemies = SMOKE_ENEMIES - 1;
    let lobby = counts.lobby;
    let defaults = selection.options()
        == sim::MatchOptions {
            starting_credits: Some(lobby.credits),
            unit_count: Some(lobby.unit_count),
            ..expected.options()
        }
        && selection.players() == expected.players()
        && !selection.map_growth
        && selection.valid(counts);
    if !defaults {
        smoke.problem(format!(
            "the menu's buttons did not restore the lobby's defaults: {selection:?}"
        ));
    }
}

/// What the match of `host`, started from the menu with `choice`, holds against what the choices
/// say: one line per difference; empty if the simulation was set up as chosen. `spawned` is what
/// match setup reported as `(units, buildings)`, `played` the countries the players play, and
/// `colours` the colour choice the match is painted with and how many different colours its
/// players have.
fn match_problems(
    choice: MenuSelection,
    catalogue: &Catalogue,
    host: &SimHost,
    played: &MatchPlayers,
    spawned: (usize, usize),
    colours: Option<(usize, usize)>,
) -> Vec<String> {
    let mut problems = Vec::new();
    let mut check = |holds: bool, what: &str| {
        if !holds {
            problems.push(what.to_owned());
        }
    };
    let setups = choice.players();
    let (Some(players), Some(state), Some(grid)) =
        (host.players(), host.match_state(), host.grid())
    else {
        return vec!["no match is set up".to_owned()];
    };
    let computers: Vec<sim::PlayerId> = setups.iter().skip(1).map(|setup| setup.player).collect();
    check(
        players.iter().count() == setups.len() && played.0.len() == setups.len(),
        "the match has other players than the menu chose",
    );
    // The extensions are the human player's alone, and every computer player is its own.
    check(
        players.get(HUMAN).is_some_and(|human| {
            human.super_me() == choice.super_me
                && human.unlock_all() == choice.unlock_all
                && human.unit_speed_percent() == u32::from(choice.unit_speed_percent())
        }),
        "Super Me, its unit speed or ALL UNLOCKED is not as chosen for the human player",
    );
    check(
        computers.iter().all(|&id| {
            players.get(id).is_some_and(|computer| {
                !computer.super_me() && !computer.unlock_all() && computer.production_speed() == 1
            }) && host.ai().is_some_and(|ai| ai.controller(id).is_some())
        }),
        "a computer player has the human player's extensions or no controller",
    );
    check(
        colours == Some((choice.color, setups.len())),
        "the players' colours are not the chosen one and one of their own for each",
    );
    // The lobby's rules.
    let rules = state.rules();
    check(
        rules
            == sim::MatchRules {
                short_game: choice.short_game,
                superweapons: choice.superweapons,
                mcv_repacks: choice.mcv_repacks,
                crates: choice.crates,
                ore_grows: true,
            },
        "short game, superweapons, MCV repacks or crates are not as chosen",
    );
    // Teams: everyone fights the human player; the computer players each other only in a free
    // for all.
    check(
        computers.iter().all(|&id| players.hostile(HUMAN, id))
            && computers.iter().all(|&a| {
                computers
                    .iter()
                    .all(|&b| a == b || players.hostile(a, b) == choice.free_for_all)
            }),
        "the teams are not as chosen",
    );
    // Countries: the chosen one, or for RANDOM some country.
    check(
        played
            .0
            .iter()
            .zip(choice.countries)
            .all(|(entry, country)| match country {
                Some(faction) => entry.faction == faction,
                None => entry.side.is_some(),
            }),
        "a player does not play the country chosen for it",
    );
    // Credits: what the human player has and what its queues were charged make up the chosen
    // starting credits (the run places no building and sells nothing).
    let lobby = MenuCounts::of(catalogue).lobby;
    let credits = choice.credits.unwrap_or(lobby.credits);
    check(
        players.get(HUMAN).is_some_and(|human| {
            let charged: i64 = human
                .queues()
                .flat_map(|(_, items)| items.iter().map(|item| item.charged))
                .sum();
            human.credits() + charged == credits
        }),
        "the human player did not start with the chosen credits",
    );
    // What the players started with: by the unit count and the MCV start.
    let unit_count = choice.unit_count.unwrap_or(lobby.unit_count);
    check(
        spawned == starting_totals(catalogue, played.factions(), unit_count, choice.mcv_start),
        "the players did not start with the units and buildings of the unit count and MCV start",
    );
    // The map: as its file defines it, or enlarged for the players.
    let size =
        crate::skirmish::match_map_size(catalogue, choice.map, setups.len(), choice.map_growth);
    let filed = catalogue
        .maps
        .get(choice.map)
        .map(|entry| (entry.map.grid.width(), entry.map.grid.height()));
    check(
        size == Some((grid.width(), grid.height())) && (choice.map_growth || size == filed),
        "the map does not have the size the menu showed",
    );
    problems
}

/// What a match holds, for [`match_problems`].
type MatchFacts<'w> = (
    Res<'w, LastMatch>,
    Res<'w, MapStatus>,
    Res<'w, SimHost>,
    Res<'w, UnitStatus>,
    Res<'w, BuildingStatus>,
    Option<Res<'w, MatchPlayers>>,
    Option<Res<'w, crate::presentation::TeamColors>>,
);

/// The human player's base units and Construction Yards in the match of `host`, and whether it
/// owns any other building.
fn human_base(host: &SimHost) -> (Vec<SimId>, Vec<SimId>, bool) {
    let rules = host.definitions().and_then(Definitions::rules);
    let world = host.world();
    let mut bases = Vec::new();
    if let Some(mut units) = world.try_query::<(&SimId, &Owner, &sim::Unit)>() {
        bases.extend(
            units
                .iter(world)
                .filter(|(_, owner, unit)| {
                    owner.0 == HUMAN && rules.is_some_and(|rules| rules.is_base_unit(unit.kind))
                })
                .map(|(id, ..)| *id),
        );
    }
    let yards: Vec<SimId> = crate::selection::own_yards(host).into_iter().collect();
    let others = world
        .try_query::<(&SimId, &Owner, &Building)>()
        .is_some_and(|mut buildings| {
            buildings
                .iter(world)
                .any(|(id, owner, _)| owner.0 == HUMAN && !yards.contains(id))
        });
    (bases, yards, others)
}

/// Whether every card the sidebar lists for the human player of `host`'s match is of an item
/// of the side of its country, and at least one card is listed: an item that the factories of
/// that side build, and no building that needs the Construction Yard of another side. With ALL
/// UNLOCKED the player's yard builds the buildings of every side, prerequisites being waived:
/// then only that some card is listed.
fn cards_of_own_side(host: &SimHost, played: Option<&MatchPlayers>) -> bool {
    let Some((definitions, rules)) = host
        .definitions()
        .and_then(|definitions| Some((definitions, definitions.rules()?)))
    else {
        return false;
    };
    let side = played.and_then(|played| played.get(HUMAN)?.side);
    let other_yards: Vec<BuildingKind> = rules
        .sides
        .iter()
        .filter(|rules| Some(rules.side) != side)
        .map(|rules| rules.construction_yard)
        .collect();
    let listed = crate::sidebar::listed_items(host);
    let unlocked = host
        .players()
        .and_then(|players| players.get(HUMAN))
        .is_some_and(PlayerState::unlock_all);
    if unlocked {
        return !listed.is_empty();
    }
    !listed.is_empty()
        && listed.iter().all(|&item| {
            definitions.metadata(item).is_none_or(|entry| {
                entry.owner_sides.is_empty()
                    || side.is_some_and(|side| entry.owner_sides.contains(&side))
            }) && definitions.production(item).is_some_and(|terms| {
                !terms
                    .prerequisites
                    .iter()
                    .any(|kind| other_yards.contains(kind))
            })
        })
}

/// Plays the MCV drill in the first match, one step per frame in which the simulation shows the
/// outcome of the step before ([`Drill`]). The commands go the interface's way: Deploy as the D
/// key issues it for the selected base unit, the repack as D or the Repack button issue it for
/// the selected Construction Yard.
fn play_drill(
    mut smoke: ResMut<SmokeTest>,
    host: Res<SimHost>,
    played: Option<Res<MatchPlayers>>,
    mut sink: ResMut<CommandSink>,
) {
    if smoke.flow != Flow::First || smoke.drill == Drill::Done {
        return;
    }
    let sees = host
        .players()
        .and_then(|players| players.get(HUMAN))
        .is_some_and(|state| state.vision().visible_cells() > 0);
    if !sees {
        return;
    }
    let (bases, yards, others) = human_base(&host);
    let deploy = |sink: &mut CommandSink| {
        sink.issue(crate::special_controls::deploy_commands(
            &crate::selection::Selection(bases.iter().copied().collect()),
        ))
    };
    let repacks = host
        .match_state()
        .is_some_and(|state| state.rules().mcv_repacks);
    match (smoke.drill, bases.len(), yards.len()) {
        (Drill::Start, 1, 0) => {
            // No building, so no card.
            if !crate::sidebar::listed_items(&host).is_empty() {
                smoke.problem("the sidebar lists cards before the Construction Yard stands");
            }
            deploy(&mut sink);
            smoke.drill = Drill::Deployed;
        }
        (Drill::Deployed, 0, 1) => {
            if others || !cards_of_own_side(&host, played.as_deref()) {
                smoke.problem(
                    "with its Construction Yard the player owns another building, or the \
                     sidebar lists no card or a card of the other side",
                );
            }
            // The yard is selected, as a left click on it selects it.
            let command = crate::special_controls::repack_command(
                yards.first().copied(),
                &yards.iter().copied().collect(),
                0,
                repacks,
            );
            if command.is_none() {
                smoke.problem("the selected Construction Yard offers no repack");
            }
            sink.issue(command);
            smoke.drill = Drill::Repacked;
        }
        (Drill::Repacked, 1, 0) => {
            if others || !crate::sidebar::listed_items(&host).is_empty() {
                smoke.problem(
                    "after the repack the player owns a building or the sidebar lists a card",
                );
            }
            deploy(&mut sink);
            smoke.drill = Drill::Redeployed;
        }
        (Drill::Redeployed, 0, 1) => {
            info!(
                "smoke test: the MCV deployed, repacked and deployed again by tick {}",
                host.tick()
            );
            smoke.drill = Drill::Done;
        }
        (step, ..) if host.tick() > DRILL_TICKS => {
            smoke.problem(format!(
                "the MCV drill stands at {step:?} after {} ticks, with {} base units and {} \
                 Construction Yards",
                host.tick(),
                bases.len(),
                yards.len()
            ));
            smoke.drill = Drill::Done;
        }
        _ => {}
    }
}

/// Plays the game flow: starts, restarts, leaves, and starts matches again, as the menu's and
/// the end screen's buttons do (their effects are computed by the same pure functions), and
/// compares every match with the choices it was started from ([`match_problems`]).
#[allow(clippy::too_many_arguments)]
fn play_flow(
    mut commands: Commands,
    mut smoke: ResMut<SmokeTest>,
    screen: Res<Screen>,
    catalogue: Option<Res<Catalogue>>,
    mut selection: ResMut<MenuSelection>,
    facts: MatchFacts,
    menu: Query<&MenuButton>,
    roots: Query<&Node, With<MenuRoot>>,
    mut speed: ResMut<crate::game_speed::GameSpeed>,
) {
    let Some(catalogue) = catalogue else {
        return;
    };
    let (last, map, host, units, buildings, played, teams) = facts;
    let long = smoke.frames >= FLOW_MIN_FRAMES;
    let counts = MenuCounts::of(&catalogue);
    let start =
        |smoke: &mut SmokeTest, commands: &mut Commands, selection: MenuSelection, next: Flow| {
            match menu_effect(MenuButton::Start, selection, counts) {
                MenuEffect::Start(choice) => {
                    commands.queue(move |world: &mut World| begin_match(world, choice, SMOKE_SEED));
                    smoke.starts += 1;
                    smoke.flow = next;
                }
                effect => {
                    error!("smoke test failed: the menu's Start button did {effect:?}");
                    smoke.rejected = true;
                    smoke.flow = Flow::Last;
                }
            }
            smoke.compared = false;
            smoke.menu_frames = 0;
        };
    let settled = map.map_created && host.tick() >= FLOW_TICKS;
    // Every match of the run holds what its menu choices say: nothing leaked from the match
    // before, nothing was lost on a restart.
    if *screen == Screen::Match
        && settled
        && !smoke.compared
        && let Some(choice) = last.0
    {
        smoke.compared = true;
        let colours = teams.as_ref().map(|teams| {
            let distinct: std::collections::BTreeSet<[u8; 3]> = choice
                .players()
                .iter()
                .map(|setup| teams.rgb_of(setup.player))
                .collect();
            (teams.0, distinct.len())
        });
        let problems = match_problems(
            choice,
            &catalogue,
            &host,
            &played.as_deref().cloned().unwrap_or_default(),
            (units.spawned, buildings.spawned),
            colours,
        );
        let (number, flow) = (smoke.starts, smoke.flow);
        for problem in problems {
            smoke.problem(format!("match {number} ({flow:?}): {problem}"));
        }
    }

    match (smoke.flow, *screen) {
        (Flow::Menu, Screen::Menu) if !long => {
            let shown = roots.iter().any(|node| node.display != Display::None);
            smoke.menu_buttons = Some(if shown { menu.iter().count() } else { 0 });
            start(&mut smoke, &mut commands, *selection, Flow::Last);
        }
        (Flow::Menu, Screen::Menu) => {
            if smoke.menu_frames == 0 {
                let shown = roots.iter().any(|node| node.display != Display::None);
                smoke.menu_buttons = Some(if shown { menu.iter().count() } else { 0 });
                // The fullest menu, for the layout check.
                smoke.saved = Some(*selection);
                *selection = fullest_menu(&catalogue, *selection);
            }
            smoke.menu_frames += 1;
            if smoke.menu_frames <= LAYOUT_FRAMES {
                return;
            }
            let problems = smoke.layout.as_ref().map_or_else(
                || vec!["the menu was not measured".to_owned()],
                |layout| layout.problems(MENU_BUTTONS),
            );
            if let Some(extent) = smoke.layout.as_ref().and_then(MenuLayout::extent) {
                info!(
                    "smoke test: the fullest menu covers {} x {} pixels (measures: {} x {}, \
                     must fit {} x {})",
                    extent.width(),
                    extent.height(),
                    menu_size().x,
                    menu_size().y,
                    MENU_FIT.x,
                    MENU_FIT.y
                );
            }
            smoke.layout_problems = Some(problems.len());
            for problem in problems {
                error!("smoke test failed: {problem}");
            }
            if let Some(saved) = smoke.saved {
                *selection = saved;
            }
            let mut choice = *selection;
            choose_first(&mut smoke, &mut choice, &catalogue, counts);
            *selection = choice;
            start(&mut smoke, &mut commands, choice, Flow::First);
        }
        (Flow::First, Screen::Match) if settled && smoke.compared && smoke.drill == Drill::Done => {
            // With MAP GROWTH on and more players than the map file lists start positions, the
            // match is played on a larger map than the file's.
            let filed = last
                .0
                .and_then(|choice| catalogue.maps.get(choice.map))
                .map_or(0, |entry| entry.map.grid.width());
            if host.grid().is_none_or(|grid| grid.width() <= filed) {
                smoke.problem("the map of the first match did not grow with its players");
            }
            match end_effect(EndButton::Restart, last.0) {
                EndEffect::Restart(choice) => {
                    commands.queue(move |world: &mut World| {
                        begin_match(world, choice, SMOKE_SEED + 1);
                    });
                    smoke.starts += 1;
                }
                EndEffect::Menu => {
                    error!("smoke test failed: Restart went to the menu");
                    smoke.rejected = true;
                }
            }
            smoke.compared = false;
            smoke.flow = Flow::Restarted;
        }
        (Flow::Restarted, Screen::Match) if settled && smoke.compared => {
            if end_effect(EndButton::Menu, last.0) == EndEffect::Menu {
                commands.queue(end_match);
            }
            smoke.flow = Flow::MenuAgain;
        }
        (Flow::MenuAgain, Screen::Menu) => {
            let mut choice = *selection;
            choose_yards(&mut smoke, &mut choice, &catalogue, counts);
            *selection = choice;
            start(&mut smoke, &mut commands, choice, Flow::Yards);
        }
        (Flow::Yards, Screen::Match) if settled && smoke.compared => {
            // The players started with their Construction Yards, the cards are listed from the
            // first tick, and with "MCV repacks" off the yard offers no repack.
            let (bases, yards, _) = human_base(&host);
            let repack = crate::special_controls::repack_command(
                yards.first().copied(),
                &yards.iter().copied().collect(),
                0,
                host.match_state()
                    .is_some_and(|state| state.rules().mcv_repacks),
            );
            if !bases.is_empty()
                || yards.len() != 1
                || repack.is_some()
                || !cards_of_own_side(&host, played.as_deref())
            {
                smoke.problem(format!(
                    "the match with the yards: {} base units, {} Construction Yards, repack \
                     {repack:?}, or the sidebar lists no card or one of the other side",
                    bases.len(),
                    yards.len()
                ));
            }
            if end_effect(EndButton::Menu, last.0) == EndEffect::Menu {
                commands.queue(end_match);
            }
            smoke.flow = Flow::MenuLast;
        }
        (Flow::MenuLast, Screen::Menu) => {
            let mut choice = *selection;
            let opening = smoke.saved.unwrap_or(choice);
            choose_last(&mut smoke, &mut choice, counts, opening);
            *selection = choice;
            // The last match runs at the fastest speed. A match starts at the speed of the
            // menu's "Game speed" row (`begin_match`); the speed itself is set as `]` pressed
            // repeatedly sets it.
            while speed.step(true) {}
            start(&mut smoke, &mut commands, choice, Flow::Last);
        }
        _ => {}
    }
}

/// Changes two options as the options screen's buttons do, once the catalogue is loaded (M7): the
/// master volume one step down and edge scrolling off. The settings file must hold them at the end.
fn change_options(
    mut smoke: ResMut<SmokeTest>,
    catalogue: Option<Res<Catalogue>>,
    mut volume: ResMut<VolumeSettings>,
    mut camera: ResMut<CameraOptions>,
) {
    if smoke.options_changed || catalogue.is_none() {
        return;
    }
    smoke.options_changed = true;
    let mut settings = Settings {
        volume: *volume,
        camera: *camera,
    };
    for button in [
        OptionsButton::Step(OptionRow::Master, false),
        OptionsButton::Switch(OptionRow::EdgeScrolling),
    ] {
        match options_effect(button, settings) {
            OptionsEffect::Set(next) => settings = next,
            effect => {
                error!("smoke test failed: the options button {button:?} did {effect:?}");
                smoke.rejected = true;
            }
        }
    }
    *volume = settings.volume;
    *camera = settings.camera;
}

/// The pause the smoke test makes (M5).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum PauseCheck {
    /// Not paused yet (or the run is too short to pause).
    #[default]
    NotYet,
    /// Paused at `frame`, after `tick` ticks; `refused` records whether a command pushed while
    /// paused was refused.
    Paused {
        frame: u32,
        tick: u64,
        refused: bool,
    },
    /// Resumed: ticks that ran while paused, and whether the command was refused.
    Done {
        ticks_while_paused: u64,
        refused: bool,
    },
}

/// Pauses the game in the middle of a long enough run, pushes a command that must be refused,
/// and resumes after [`PAUSE_FRAMES`] frames.
fn check_pause(
    mut smoke: ResMut<SmokeTest>,
    mut sink: ResMut<CommandSink>,
    host: Res<SimHost>,
    speed: Res<crate::game_speed::GameSpeed>,
    fixed: Res<Time<Fixed>>,
) {
    if smoke.frames < PAUSE_MIN_FRAMES || smoke.flow != Flow::Last {
        return;
    }
    let frame = smoke.rendered;
    match smoke.pause {
        PauseCheck::NotYet if frame >= smoke.frames / 2 => {
            // The game speed drives the simulation's fixed timestep (the pause must still hold
            // at it).
            let expected = std::time::Duration::from_secs_f64(1.0 / speed.ticks_per_second());
            if speed.factor() != 4.0 || fixed.timestep() != expected {
                error!(
                    "smoke test failed: game speed {} with a fixed timestep of {:?}",
                    speed.label(),
                    fixed.timestep()
                );
                smoke.rejected = true;
            }
            if !sink.paused() {
                sink.toggle_pause();
            }
            // A command that the simulation would reject (it names no unit), so that the run
            // fails if the sink accepted it while paused.
            let refused =
                sink.push(Command::Stop { units: Vec::new() }) == Err(CommandRefused::Paused);
            info!("smoke test: paused at tick {}", host.tick());
            smoke.pause = PauseCheck::Paused {
                frame,
                tick: host.tick(),
                refused,
            };
        }
        PauseCheck::Paused {
            frame: since,
            tick,
            refused,
        } if frame >= since.saturating_add(PAUSE_FRAMES) => {
            if sink.paused() {
                sink.toggle_pause();
            }
            info!("smoke test: resumed at tick {}", host.tick());
            smoke.pause = PauseCheck::Done {
                ticks_while_paused: host.tick().saturating_sub(tick),
                refused,
            };
        }
        _ => {}
    }
}

/// The first building of the production menu that `owned` (the player's building types) lets
/// the player with `state` queue: `QueueProduction` of it passes the buildability rule
/// (`sim::check_buildable`: roster, prerequisites, factory, side and country; RA1).
pub fn first_queueable(
    definitions: &Definitions,
    owned: &[BuildingKind],
    state: &PlayerState,
) -> Option<ProductionItem> {
    production_menu(definitions).into_iter().find(|&item| {
        matches!(item, ProductionItem::Building(_))
            && sim::check_buildable(definitions, state, true, owned, item).is_ok()
    })
}

/// Queues the first building the player can produce once it owns a building (its Construction
/// Yard: a starting building, or since RA1 the base unit the run has deployed, [`order_move`]),
/// as a left click on its sidebar button does.
fn order_production(
    mut smoke: ResMut<SmokeTest>,
    host: Res<SimHost>,
    mut sink: ResMut<CommandSink>,
) {
    if smoke.production.is_some() || smoke.flow != Flow::Last {
        return;
    }
    let (Some(definitions), Some(state)) = (
        host.definitions(),
        host.players().and_then(|players| players.get(HUMAN)),
    ) else {
        return;
    };
    let owned: Vec<BuildingKind> = host
        .world()
        .try_query::<(&Owner, &Building)>()
        .map(|mut buildings| {
            buildings
                .iter(host.world())
                .filter(|(owner, _)| owner.0 == HUMAN)
                .map(|(_, building)| building.kind)
                .collect()
        })
        .unwrap_or_default();
    if owned.is_empty() {
        return;
    }
    let item = first_queueable(definitions, &owned, state);
    if let Some(item) = item {
        let menu = production_menu(definitions);
        let item_state = item_status(
            item,
            definitions,
            &owned.iter().copied().collect(),
            state,
            host.match_state()
                .is_none_or(|state| state.rules().superweapons),
        );
        match button_effect(
            SidebarButton::Item(item),
            MouseButton::Left,
            &menu,
            item_state,
            InterfaceMode::Normal,
        ) {
            SidebarEffect::Commands(commands) => {
                sink.issue(commands);
            }
            effect => {
                error!("smoke test failed: the sidebar button of {item:?} did {effect:?}");
                smoke.rejected = true;
            }
        }
    }
    smoke.production = Some((host.tick(), item));
}

/// Orders all of the player's units to [`MOVE_TARGET`] once they exist, as a right click with
/// all of them selected does. Since RA1 the player starts with the base unit of its side (an
/// MCV) instead of a Construction Yard: the run deploys it first, as the D key does with it
/// selected, and moves the others. Both wait for the first tick, after which the player sees
/// the ground the yard will stand on, which `Deploy` requires.
fn order_move(
    mut smoke: ResMut<SmokeTest>,
    status: Res<UnitStatus>,
    host: Res<SimHost>,
    mut sink: ResMut<CommandSink>,
) {
    if smoke.order.is_some() || status.spawned == 0 || smoke.flow != Flow::Last {
        return;
    }
    let sees = host
        .players()
        .and_then(|players| players.get(HUMAN))
        .is_some_and(|state| state.vision().visible_cells() > 0);
    if !sees {
        return;
    }
    let base_unit = |kind: sim::UnitKind| {
        host.definitions()
            .and_then(Definitions::rules)
            .is_some_and(|rules| rules.is_base_unit(kind))
    };
    let mut bases = std::collections::BTreeSet::new();
    let mut start: BTreeMap<SimId, SubPos> = BTreeMap::new();
    if let Some(mut units) = host
        .world()
        .try_query::<(&SimId, &Owner, &sim::Unit, &Position)>()
    {
        for (id, owner, unit, position) in units.iter(host.world()) {
            if owner.0 != HUMAN {
                continue;
            }
            if base_unit(unit.kind) {
                bases.insert(*id);
            } else {
                start.insert(*id, position.0);
            }
        }
    }
    sink.issue(crate::special_controls::deploy_commands(
        &crate::selection::Selection(bases),
    ));
    let selection = start.keys().copied().collect();
    sink.issue(crate::selection::move_command(
        &selection,
        Some(MOVE_TARGET),
    ));
    smoke.order = Some((host.tick(), start));
}

fn watch_events(mut smoke: ResMut<SmokeTest>, mut events: MessageReader<SimEvent>) {
    for event in events.read() {
        // The smoke test's own commands are the human player's. The computer player's rejections
        // are logged by `crate::units` and counted (M6 review 1 P2-4).
        if let SimEvent::CommandRejected { player, reason, .. } = event {
            if *player == HUMAN {
                error!("smoke test failed: a command of the smoke test was rejected: {reason}");
                smoke.rejected = true;
            } else {
                // Every computer player of the match, not only the first.
                smoke.ai_rejected = smoke.ai_rejected.saturating_add(1);
            }
        }
    }
}

/// Whether a run that exits after `rendered` of `frames` frames ended before its checks ran (for
/// example because its window was closed, which makes Bevy exit with success).
pub fn exits_early(rendered: u32, frames: u32) -> bool {
    rendered < frames
}

/// Turns an exit before the last frame (Bevy exits with success when no window is left, for
/// example when a display is removed) into a failure, so that a run passes only after its checks
/// (M6; found when a debug smoke run exited with code 0 after a monitor was removed).
fn fail_on_early_exit(smoke: Res<SmokeTest>, mut exits: ResMut<Messages<AppExit>>) {
    let exiting = exits.iter_current_update_messages().next().is_some();
    if exiting && exits_early(smoke.rendered, smoke.frames) {
        error!(
            "smoke test failed: the application exited after {} of {} frames",
            smoke.rendered, smoke.frames
        );
        exits.write(AppExit::error());
    }
}

fn fail_on_asset_load_error(
    mut failures: MessageReader<UntypedAssetLoadFailedEvent>,
    mut exit: MessageWriter<AppExit>,
) {
    for failure in failures.read() {
        error!(
            "smoke test failed: asset {} failed to load: {}",
            failure.path, failure.error
        );
        exit.write(AppExit::error());
    }
}

/// The views, for [`view_mismatches`].
type ViewQueries<'w, 's> = (
    Query<'w, 's, (&'static UnitView, &'static Visibility)>,
    Query<'w, 's, (&'static BuildingView, &'static Visibility)>,
);

/// The unit and building views whose visibility disagrees with what the human player sees
/// (`crate::units::unit_shown`, `crate::buildings::building_shown`), and how many views of other
/// players' units and buildings are shown.
fn view_mismatches(
    players: &Players,
    world: &World,
    unit_views: &Query<(&UnitView, &Visibility)>,
    building_views: &Query<(&BuildingView, &Visibility)>,
) -> (usize, usize) {
    let mut mismatches = 0;
    let mut enemies_shown = 0;
    for (view, visibility) in unit_views {
        let Some(mobile) = world.get::<Mobile>(view.unit) else {
            continue;
        };
        let shown = *visibility != Visibility::Hidden;
        mismatches += usize::from(shown != unit_shown(view.owner, mobile, players));
        enemies_shown += usize::from(shown && view.owner != HUMAN);
    }
    for (view, visibility) in building_views {
        let (Some(owner), Some(building)) = (
            world.get::<Owner>(view.building),
            world.get::<Building>(view.building),
        ) else {
            continue;
        };
        let shown = *visibility != Visibility::Hidden;
        mismatches += usize::from(shown != building_shown(owner.0, building.footprint, players));
        enemies_shown += usize::from(shown && owner.0 != HUMAN);
    }
    (mismatches, enemies_shown)
}

/// What the interface shows, for the smoke test's M5 checks.
type InterfaceQueries<'w, 's> = (
    Query<
        'w,
        's,
        (
            &'static SidebarButton,
            &'static ComputedNode,
            &'static UiGlobalTransform,
        ),
    >,
    Query<'w, 's, (&'static Text, &'static SidebarText)>,
    Query<'w, 's, (), With<MusicPlayer>>,
    Single<'w, 's, &'static Window, With<bevy::window::PrimaryWindow>>,
);

/// Whether a laid-out node lies inside a window of `size` logical pixels; nodes that are not
/// displayed (size 0) count as inside.
fn node_inside(node: &ComputedNode, transform: &UiGlobalTransform, size: Vec2) -> bool {
    let scale = node.inverse_scale_factor();
    let half = node.size() * scale / 2.0;
    if half == Vec2::ZERO {
        return true;
    }
    let centre = transform.translation * scale;
    let (min, max) = (centre - half, centre + half);
    min.x >= -0.5 && min.y >= -0.5 && max.x <= size.x + 0.5 && max.y <= size.y + 0.5
}

#[allow(clippy::too_many_arguments)]
fn count_frames(
    mut smoke: ResMut<SmokeTest>,
    map: Res<MapStatus>,
    status: Res<UnitStatus>,
    buildings: Res<BuildingStatus>,
    host: Res<SimHost>,
    minimap: Res<MinimapState>,
    sounds: Option<Res<SoundBank>>,
    views: ViewQueries,
    interface: InterfaceQueries,
    flow: (
        Option<Res<Catalogue>>,
        Res<LastMatch>,
        Option<Res<MatchPlayers>>,
    ),
    options: OptionQueries,
    mut exit: MessageWriter<AppExit>,
) {
    smoke.rendered = smoke.rendered.saturating_add(1);
    // The checks run once, at the last frame. The runner exits after it, but may update once more
    // first (`bevy_winit` checks for `AppExit` only when its event loop is about to wait); the
    // exit message of the last frame is still read then.
    if smoke.rendered != smoke.frames {
        return;
    }
    let (catalogue, last, played) = flow;
    // What the players of the last match started with: by the countries they play (the
    // simulation has drawn the RANDOM ones), the unit count and the MCV start of its choices.
    let (expected_units, expected_buildings) = match (catalogue.as_deref(), last.0, &played) {
        (Some(catalogue), Some(selection), Some(played)) => starting_totals(
            catalogue,
            played.factions(),
            selection
                .unit_count
                .unwrap_or(MenuCounts::of(catalogue).lobby.unit_count),
            selection.mcv_start,
        ),
        _ => (0, 0),
    };
    let empty = Players::default();
    let players = host.players().unwrap_or(&empty);
    let tick = host.tick();
    let (unit_views, building_views) = views;
    let (mismatches, enemies_shown) =
        view_mismatches(players, host.world(), &unit_views, &building_views);
    let visible_cells = players
        .get(HUMAN)
        .map_or(0, |state| state.vision().visible_cells());
    let positions: BTreeMap<SimId, SubPos> = host
        .world()
        .try_query::<(&SimId, &Position)>()
        .map(|mut units| {
            units
                .iter(host.world())
                .map(|(id, position)| (*id, position.0))
                .collect()
        })
        .unwrap_or_default();
    let (ticks_since_order, moved) = match &smoke.order {
        Some((ordered_at, start)) => (
            tick.saturating_sub(*ordered_at),
            positions
                .iter()
                .filter(|(id, position)| start.get(id).is_some_and(|at| at != *position))
                .count(),
        ),
        None => (0, 0),
    };
    let (sidebar_buttons, sidebar_texts, music, window) = interface;
    let expected_items = host
        .definitions()
        .map_or(0, |definitions| production_menu(definitions).len());
    let sidebar_items = sidebar_buttons
        .iter()
        .filter(|(button, ..)| matches!(button, SidebarButton::Item(_)))
        .count();
    let buttons_outside = sidebar_buttons
        .iter()
        .filter(|(_, node, transform)| !node_inside(node, transform, window.size()))
        .count();
    // M7: displayed item buttons whose item the human player's faction does not offer.
    let off_roster_shown = players.get(HUMAN).map_or(0, |state| {
        sidebar_buttons
            .iter()
            .filter(|(button, node, _)| {
                matches!(button, SidebarButton::Item(item) if !state.offers(*item))
                    && node.size() != Vec2::ZERO
            })
            .count()
    });
    // RA1: the cards displayed are among those the sidebar's rule lists (what some building of
    // the player could build), and some card is displayed once the player owns a building.
    let listed = crate::sidebar::listed_items(&host);
    let cards = |wanted: bool| {
        sidebar_buttons
            .iter()
            .filter(|(button, node, _)| {
                matches!(button, SidebarButton::Item(item) if listed.contains(item) == wanted)
                    && node.size() != Vec2::ZERO
            })
            .count()
    };
    let (cards_shown, unlisted_shown) = (cards(true), cards(false));
    let owns_building =
        host.world()
            .try_query::<(&Owner, &Building)>()
            .is_some_and(|mut buildings| {
                buildings
                    .iter(host.world())
                    .any(|(owner, _)| owner.0 == HUMAN)
            });
    // M7: the options written to the run's settings file read back unchanged.
    let (settings_file, volume, camera, options_buttons) = options;
    let current = Settings {
        volume: *volume,
        camera: *camera,
    };
    let settings_saved = settings_file.writes > 0
        && settings_file
            .path
            .as_deref()
            .is_some_and(|path| read_settings(path).ok().flatten() == Some(current));
    if let Some(dir) = settings_file
        .path
        .as_deref()
        .and_then(std::path::Path::parent)
    {
        let _ = std::fs::remove_dir_all(dir);
    }
    let options_buttons = options_buttons.iter().count();
    let credits_shown = sidebar_texts
        .iter()
        .find(|(_, kind)| **kind == SidebarText::Credits)
        .map(|(text, _)| text.0.clone());
    let credits_expected = credits_text(players.get(HUMAN));
    let sounds = sounds.map_or(0, |bank| bank.effects.len());
    // Credits charged for everything in the player's queues.
    let charged: i64 = players.get(HUMAN).map_or(0, |state| {
        state
            .queues()
            .flat_map(|(_, items)| items.iter().map(|item| item.charged))
            .sum()
    });
    let (production_ordered, ticks_since_production) = match smoke.production {
        Some((at, item)) => (item.is_some(), tick.saturating_sub(at)),
        None => (false, 0),
    };
    let count = |world: &World, buildings: bool| {
        world
            .try_query::<(&SimId, Option<&Building>)>()
            .map_or(0, |mut entities| {
                entities
                    .iter(world)
                    .filter(|(_, building)| building.is_some() == buildings)
                    .count()
            })
    };
    let (sim_units, sim_buildings) = (count(host.world(), false), count(host.world(), true));
    let (unit_view_count, building_view_count) =
        (unit_views.iter().count(), building_views.iter().count());
    // The least active computer player: every one of them must have issued commands.
    let ai_commands = host
        .match_state()
        .map(|state| {
            state
                .players()
                .iter()
                .filter(|&&id| id != HUMAN)
                .map(|&id| {
                    host.ai()
                        .and_then(|ai| ai.controller(id))
                        .map_or(0, |controller| controller.commands_issued())
                })
                .min()
                .unwrap_or(0)
        })
        .unwrap_or(0);
    info!(
        "smoke test: {} frames, {} simulation ticks, map created: {}, units spawned: {}, \
         units moved: {moved} in {ticks_since_order} ticks after the order, buildings spawned: \
         {}, production charged {charged} credits in {ticks_since_production} ticks, visible \
         cells: {visible_cells}, enemy views shown: {enemies_shown}, view mismatches: \
         {mismatches}, sidebar items: {sidebar_items} of {expected_items}, sidebar buttons outside the \
         window: {buttons_outside}, sidebar credits: \
         {credits_shown:?}, minimap drawings: {}, minimap pixels lit: {}, sounds: {}, music \
         players: {}, pause: {:?}, menu buttons: {:?}, matches started: {}, flow: {:?}, views: \
         {unit_view_count} of {sim_units} units and {building_view_count} of {sim_buildings} \
         buildings, computer player's commands: {ai_commands} ({} rejected), items outside the \
         roster shown: {off_roster_shown}, cards shown: {cards_shown} of {} listed \
         ({unlisted_shown} not listed), options buttons: {options_buttons}, settings saved: \
         {settings_saved}, menu layout problems: {:?}, game flow problems: {}",
        smoke.rendered,
        tick,
        map.map_created,
        status.spawned,
        buildings.spawned,
        minimap.drawings,
        minimap.lit_pixels,
        sounds,
        music.iter().count(),
        smoke.pause,
        smoke.menu_buttons,
        smoke.starts,
        smoke.flow,
        smoke.ai_rejected,
        listed.len(),
        smoke.layout_problems,
        smoke.flow_problems
    );
    let failures = smoke_failures(&SmokeOutcome {
        frames: smoke.frames,
        map_created: map.map_created,
        spawned: status.spawned,
        expected_units,
        rejected: smoke.rejected,
        ticks_since_order,
        moved,
        buildings: buildings.spawned,
        expected_buildings,
        // Nothing is queued before the player owns a building: its base unit must have deployed
        // into the Construction Yard by the time a unit must have moved.
        production_ordered: match smoke.production {
            Some(_) => production_ordered,
            None => ticks_since_order < MOVE_CHECK_TICKS,
        },
        ticks_since_production,
        charged,
        ticks: tick,
        visible_cells,
        view_mismatches: mismatches,
        sidebar_items,
        expected_items,
        buttons_outside,
        credits_text_shown: credits_shown.as_deref() == Some(credits_expected.as_str()),
        minimap_drawings: minimap.drawings,
        minimap_lit_pixels: minimap.lit_pixels,
        // RA2: the drawing of this tick shows the map exactly if the player has radar. What
        // was drawn is asked (the pixels of the image that show something of the map), not
        // the flag the drawing sets from the same radar (advisory review 1, tests P1-1).
        minimap_follows_radar: minimap.drawn_tick != Some(tick)
            || (minimap.map_pixels > 0) == crate::minimap::radar_online(&host),
        sounds_complete: sounds == Sfx::ALL.len(),
        music_players: music.iter().count(),
        pause: match smoke.pause {
            PauseCheck::NotYet => (smoke.frames < PAUSE_MIN_FRAMES).then_some((0, true)),
            PauseCheck::Paused { .. } => None,
            PauseCheck::Done {
                ticks_while_paused,
                refused,
            } => Some((ticks_while_paused, refused)),
        },
        menu_buttons: smoke.menu_buttons.unwrap_or(0),
        flow_done: smoke.flow == Flow::Last,
        starts: smoke.starts,
        expected_starts: if smoke.frames >= FLOW_MIN_FRAMES {
            LONG_RUN_STARTS
        } else {
            1
        },
        views: (unit_view_count, building_view_count),
        simulated: (sim_units, sim_buildings),
        ai_commands,
        ai_rejected: smoke.ai_rejected,
        off_roster_shown,
        unlisted_shown,
        cards_missing: owns_building && tick > 0 && cards_shown == 0,
        options_buttons,
        settings_saved,
        // A run long enough for the game flow must have checked the menu's layout.
        layout_problems: smoke
            .layout_problems
            .or((smoke.frames < FLOW_MIN_FRAMES).then_some(0)),
        flow_problems: smoke.flow_problems,
    });
    if failures.is_empty() {
        exit.write(AppExit::Success);
    } else {
        for failure in failures {
            error!("smoke test failed: {failure}");
        }
        exit.write(AppExit::error());
    }
}

/// What a smoke run observed by its last frame.
#[derive(Debug, Clone, Copy)]
struct SmokeOutcome {
    frames: u32,
    map_created: bool,
    spawned: usize,
    /// The configured number of starting units.
    expected_units: usize,
    rejected: bool,
    ticks_since_order: u64,
    moved: usize,
    buildings: usize,
    /// The configured number of starting buildings.
    expected_buildings: usize,
    /// `false` if the player's buildings let it queue nothing, or if it still owns none
    /// [`MOVE_CHECK_TICKS`] ticks after the order that deploys its base unit (RA1: it starts
    /// without buildings).
    production_ordered: bool,
    ticks_since_production: u64,
    /// Credits charged so far for the player's queued items.
    charged: i64,
    /// Simulation ticks run.
    ticks: u64,
    /// Cells the human player sees.
    visible_cells: usize,
    /// Views shown although the human player does not see what they draw, or hidden although
    /// it does.
    view_mismatches: usize,
    /// Production buttons of the sidebar (M5).
    sidebar_items: usize,
    /// Entries of the production menu.
    expected_items: usize,
    /// Sidebar buttons that are displayed but do not lie inside the window.
    buttons_outside: usize,
    /// The sidebar's credits show the human player's credits.
    credits_text_shown: bool,
    /// Drawings of the minimap, and its pixels that are not black in the last one.
    minimap_drawings: u64,
    minimap_lit_pixels: usize,
    /// The minimap shows the map while the human player has radar and the "radar offline"
    /// screen while it has none (RA2).
    minimap_follows_radar: bool,
    /// Every sound effect was synthesised.
    sounds_complete: bool,
    /// Entities playing the music.
    music_players: usize,
    /// The pause: ticks that ran while paused, and whether a command pushed meanwhile was
    /// refused; `None` if a run long enough to pause did not complete its pause, and
    /// `Some((0, true))` for a run too short to pause.
    pause: Option<(u64, bool)>,
    /// Buttons of the menu shown when the run started (M6).
    menu_buttons: usize,
    /// Whether the game flow reached the last match.
    flow_done: bool,
    /// Matches started, and how many the run's length calls for.
    starts: u32,
    expected_starts: u32,
    /// Unit and building views, and the simulation's units and buildings.
    views: (usize, usize),
    simulated: (usize, usize),
    /// Commands the computer player pushed in the last match.
    ai_commands: u64,
    /// Commands of the computer player rejected in any match of the run.
    ai_rejected: u64,
    /// Displayed sidebar buttons of items outside the human player's roster (M7).
    off_roster_shown: usize,
    /// Displayed sidebar cards of items that the sidebar's rule does not list: that no building
    /// of the human player could build (RA1, `crate::sidebar::item_listed`).
    unlisted_shown: usize,
    /// The human player owns a building and no card is displayed.
    cards_missing: bool,
    /// Problems of the menu's layout with its fullest texts (RA1); `None` if a run long enough
    /// for the game flow did not check it.
    layout_problems: Option<usize>,
    /// Checks of the game flow that failed (RA1): matches that did not hold what their menu
    /// choices said, steps of the MCV drill that did not happen.
    flow_problems: usize,
    /// Buttons of the options screen (M7).
    options_buttons: usize,
    /// The options the run changed were written and read back unchanged (M7).
    settings_saved: bool,
}

/// Buttons of the menu: previous and next for each of its rows (those of the two columns and of
/// the player list), Start, Options (M7), and Quit.
pub const MENU_BUTTONS: usize = crate::skirmish::Row::COUNT * 2 + 3;

/// Matches a run long enough for the game flow starts: the first with the options changed, its
/// restart, the one with the Construction Yards, and the last.
pub const LONG_RUN_STARTS: u32 = 4;

/// What the options screen and the settings file show, for the smoke test's M7 checks.
type OptionQueries<'w, 's> = (
    Res<'w, SettingsFile>,
    Res<'w, VolumeSettings>,
    Res<'w, CameraOptions>,
    Query<'w, 's, &'static OptionsButton>,
);

/// The reasons a smoke run fails; empty if it passes.
fn smoke_failures(outcome: &SmokeOutcome) -> Vec<String> {
    let mut failures = Vec::new();
    if !outcome.map_created {
        failures.push(format!(
            "the map was not created within {} frames",
            outcome.frames
        ));
    }
    if outcome.spawned != outcome.expected_units {
        failures.push(format!(
            "{} of {} starting units were spawned",
            outcome.spawned, outcome.expected_units
        ));
    }
    if outcome.buildings != outcome.expected_buildings {
        failures.push(format!(
            "{} of {} starting buildings were spawned",
            outcome.buildings, outcome.expected_buildings
        ));
    }
    if outcome.rejected {
        failures.push("a command of the smoke test was rejected".to_string());
    }
    if !outcome.production_ordered {
        failures.push(
            "the player's base unit did not deploy, or its buildings let it queue nothing"
                .to_string(),
        );
    }
    if outcome.ticks_since_production >= PRODUCTION_CHECK_TICKS && outcome.charged == 0 {
        failures.push(format!(
            "nothing was charged for the queued production within {} ticks",
            outcome.ticks_since_production
        ));
    }
    if outcome.ticks_since_order >= MOVE_CHECK_TICKS && outcome.moved == 0 {
        failures.push(format!(
            "no unit moved within {} ticks of the order",
            outcome.ticks_since_order
        ));
    }
    if outcome.ticks > 0 && outcome.visible_cells == 0 {
        failures.push(format!(
            "the human player sees no cell after {} ticks",
            outcome.ticks
        ));
    }
    if outcome.view_mismatches > 0 {
        failures.push(format!(
            "{} views are shown or hidden against what the human player sees",
            outcome.view_mismatches
        ));
    }
    if outcome.sidebar_items != outcome.expected_items || outcome.expected_items == 0 {
        failures.push(format!(
            "the sidebar has {} of {} production buttons",
            outcome.sidebar_items, outcome.expected_items
        ));
    }
    if outcome.buttons_outside > 0 {
        failures.push(format!(
            "{} sidebar buttons lie outside the window",
            outcome.buttons_outside
        ));
    }
    if !outcome.credits_text_shown {
        failures.push("the sidebar does not show the player's credits".to_string());
    }
    if outcome.ticks > 0 && (outcome.minimap_drawings == 0 || outcome.minimap_lit_pixels == 0) {
        failures.push(format!(
            "the minimap was drawn {} times with {} lit pixels",
            outcome.minimap_drawings, outcome.minimap_lit_pixels
        ));
    }
    if !outcome.minimap_follows_radar {
        failures.push("the minimap does not follow the player's radar".to_string());
    }
    if !outcome.sounds_complete {
        failures.push("not every sound effect was synthesised".to_string());
    }
    if outcome.music_players != 1 {
        failures.push(format!(
            "{} entities play the music, expected 1",
            outcome.music_players
        ));
    }
    if outcome.menu_buttons != MENU_BUTTONS {
        failures.push(format!(
            "the menu showed {} of {MENU_BUTTONS} buttons",
            outcome.menu_buttons
        ));
    }
    if !outcome.flow_done || outcome.starts != outcome.expected_starts {
        failures.push(format!(
            "{} of {} matches started, the game flow did not reach the last one",
            outcome.starts, outcome.expected_starts
        ));
    }
    if outcome.ticks > 0 && outcome.views != outcome.simulated {
        failures.push(format!(
            "{} unit and {} building views for {} units and {} buildings",
            outcome.views.0, outcome.views.1, outcome.simulated.0, outcome.simulated.1
        ));
    }
    if outcome.ticks >= AI_CHECK_TICKS && outcome.ai_commands == 0 {
        failures.push(format!(
            "the computer player issued no command in {} ticks",
            outcome.ticks
        ));
    }
    // A short match without contact between the players explains no rejection (D43 item 5), so
    // every command the computer player issued must have been accepted.
    if outcome.ai_rejected > 0 {
        failures.push(format!(
            "{} commands of the computer player were rejected",
            outcome.ai_rejected
        ));
    }
    if outcome.off_roster_shown > 0 {
        failures.push(format!(
            "{} sidebar buttons of items outside the roster of the human player's faction are \
             shown",
            outcome.off_roster_shown
        ));
    }
    if outcome.unlisted_shown > 0 {
        failures.push(format!(
            "{} sidebar cards are shown of items that no building of the player could build",
            outcome.unlisted_shown
        ));
    }
    if outcome.cards_missing {
        failures.push("the player owns a building and the sidebar shows no card".to_string());
    }
    match outcome.layout_problems {
        None => failures.push("the layout of the menu was not checked".to_string()),
        Some(0) => {}
        Some(problems) => failures.push(format!(
            "the layout of the menu has {problems} problems (logged above)"
        )),
    }
    if outcome.flow_problems > 0 {
        failures.push(format!(
            "{} checks of the game flow failed (logged above)",
            outcome.flow_problems
        ));
    }
    if outcome.options_buttons != OPTIONS_BUTTONS {
        failures.push(format!(
            "the options screen has {} of {OPTIONS_BUTTONS} buttons",
            outcome.options_buttons
        ));
    }
    if !outcome.settings_saved {
        failures.push(
            "the options the run changed were not written to its settings file and read back"
                .to_string(),
        );
    }
    match outcome.pause {
        None => failures.push("the pause did not end within the run".to_string()),
        Some((ticks, _)) if ticks > 0 => {
            failures.push(format!("{ticks} ticks ran while the game was paused"));
        }
        Some((_, false)) => {
            failures.push("a command was accepted while the game was paused".to_string());
        }
        Some(_) => {}
    }
    failures
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;

    const PASSING: SmokeOutcome = SmokeOutcome {
        frames: 600,
        map_created: true,
        spawned: 26,
        expected_units: 26,
        rejected: false,
        ticks_since_order: 198,
        moved: 18,
        buildings: 2,
        expected_buildings: 2,
        production_ordered: true,
        ticks_since_production: 198,
        charged: 396,
        ticks: 198,
        visible_cells: 400,
        view_mismatches: 0,
        sidebar_items: 9,
        expected_items: 9,
        buttons_outside: 0,
        credits_text_shown: true,
        minimap_drawings: 198,
        minimap_lit_pixels: 1000,
        minimap_follows_radar: true,
        sounds_complete: true,
        music_players: 1,
        pause: Some((0, true)),
        menu_buttons: MENU_BUTTONS,
        flow_done: true,
        starts: LONG_RUN_STARTS,
        expected_starts: LONG_RUN_STARTS,
        views: (26, 2),
        simulated: (26, 2),
        ai_commands: 4,
        ai_rejected: 0,
        off_roster_shown: 0,
        unlisted_shown: 0,
        cards_missing: false,
        layout_problems: Some(0),
        flow_problems: 0,
        options_buttons: OPTIONS_BUTTONS,
        settings_saved: true,
    };

    #[test]
    fn a_run_passes_only_with_the_roster_shown_and_the_options_stored() {
        // M7: an item outside the human player's roster shown in the sidebar, an options screen
        // without all of its buttons, or options that were not written and read back each fail
        // the run.
        let fails = |outcome: SmokeOutcome| smoke_failures(&outcome);
        assert_eq!(
            fails(SmokeOutcome {
                off_roster_shown: 1,
                ..PASSING
            }),
            [
                "1 sidebar buttons of items outside the roster of the human player's faction are \
                 shown"
            ]
        );
        for options_buttons in [0, OPTIONS_BUTTONS - 1, OPTIONS_BUTTONS + 1] {
            assert_eq!(
                fails(SmokeOutcome {
                    options_buttons,
                    ..PASSING
                }),
                [format!(
                    "the options screen has {options_buttons} of {OPTIONS_BUTTONS} buttons"
                )]
            );
        }
        assert_eq!(
            fails(SmokeOutcome {
                settings_saved: false,
                ..PASSING
            }),
            ["the options the run changed were not written to its settings file and read back"]
        );
        // The run's settings file lies in the temporary directory, never in the player's.
        let path = settings_path();
        assert!(path.starts_with(std::env::temp_dir()), "{}", path.display());
        assert_ne!(Some(path), crate::options::default_settings_path());
    }

    #[test]
    fn a_run_passes_only_with_every_configured_starting_unit() {
        assert_eq!(smoke_failures(&PASSING), Vec::<String>::new());
        // M2 review 1, P1-2: a map that holds only some of the starting units fails the run.
        for spawned in [0, 12, 25, 27] {
            let failures = smoke_failures(&SmokeOutcome { spawned, ..PASSING });
            assert_eq!(
                failures,
                vec![format!("{spawned} of 26 starting units were spawned")]
            );
        }
        assert_eq!(
            smoke_failures(&SmokeOutcome {
                map_created: false,
                rejected: true,
                moved: 0,
                ..PASSING
            })
            .len(),
            3
        );
        // Too few ticks after the order to require movement.
        assert!(
            smoke_failures(&SmokeOutcome {
                ticks_since_order: MOVE_CHECK_TICKS - 1,
                moved: 0,
                ..PASSING
            })
            .is_empty()
        );
    }

    #[test]
    fn a_run_passes_only_with_its_buildings_and_charged_production() {
        assert_eq!(
            smoke_failures(&SmokeOutcome {
                buildings: 1,
                ..PASSING
            }),
            ["1 of 2 starting buildings were spawned"]
        );
        assert_eq!(
            smoke_failures(&SmokeOutcome {
                production_ordered: false,
                ..PASSING
            }),
            ["the player's base unit did not deploy, or its buildings let it queue nothing"]
        );
        assert_eq!(
            smoke_failures(&SmokeOutcome {
                charged: 0,
                ..PASSING
            }),
            ["nothing was charged for the queued production within 198 ticks"]
        );
        // Too few ticks after the order to require a charge.
        assert!(
            smoke_failures(&SmokeOutcome {
                ticks_since_production: PRODUCTION_CHECK_TICKS - 1,
                charged: 0,
                ..PASSING
            })
            .is_empty()
        );
    }

    #[test]
    fn the_demo_player_can_queue_a_building_with_its_starting_buildings() {
        // M6: the human player's starting buildings are those of its faction in the skirmish file
        // (the menu's first choice, D40).
        // M7: for every faction, and only items its roster offers.
        // RA1: the shipped factions are countries. A country's player has no roster and starts
        // with the base unit of its side: what it owns once the run has deployed it is the yard,
        // which every building of its side names as a prerequisite. The factions with rosters
        // and starting buildings are those of the legacy ruleset fixture.
        let catalogue = crate::skirmish::tests::production_catalogue();
        let selection = MenuSelection::initial(&catalogue);
        let definitions = &catalogue.definitions;
        assert!(selection.faction() < catalogue.skirmish.factions.len());
        let rules = definitions.rules().expect("rules");
        for (index, faction) in catalogue.skirmish.factions.iter().enumerate() {
            let side = faction.side.and_then(|side| rules.side(side));
            let owned = vec![side.expect("a country").construction_yard];
            let mut players = Players::default();
            players.add(sim::PlayerId(0), 0).expect("player");
            assert!(players.set_country(sim::PlayerId(0), faction.country));
            let state = players.get(sim::PlayerId(0)).expect("player");
            assert_eq!(state.country(), faction.country);
            let item = first_queueable(definitions, &owned, state).expect("something to queue");
            assert!(faction.offers(item), "faction {index}: {item:?}");
            let terms = definitions.production(item).expect("terms");
            assert!(terms.prerequisites.iter().all(|kind| owned.contains(kind)));
            assert_eq!(sim::missing_prerequisite(definitions, &owned, item), None);
            // At least one credit is charged on the first tick, so the smoke check can see it.
            assert!(terms.cost / i64::from(terms.build_time) > 0 || terms.build_time == 1);
            assert_eq!(first_queueable(definitions, &[], state), None);
            // An empty roster offers nothing.
            let mut nothing = Players::default();
            nothing
                .add_with_roster(sim::PlayerId(0), 0, Default::default())
                .expect("player");
            let state = nothing.get(sim::PlayerId(0)).expect("player");
            assert_eq!(first_queueable(definitions, &owned, state), None);
        }
        let legacy = crate::skirmish::tests::legacy_catalogue();
        let definitions = &legacy.definitions;
        assert_eq!(legacy.skirmish.factions.len(), 2);
        for (index, faction) in legacy.skirmish.factions.iter().enumerate() {
            let owned: Vec<BuildingKind> = faction
                .buildings
                .iter()
                .map(|building| building.kind)
                .collect();
            let mut players = Players::default();
            players
                .add_with_roster(sim::PlayerId(0), 0, faction.roster.clone())
                .expect("player");
            let state = players.get(sim::PlayerId(0)).expect("player");
            let item = first_queueable(definitions, &owned, state).expect("something to queue");
            assert!(faction.offers(item), "faction {index}: {item:?}");
            let terms = definitions.production(item).expect("terms");
            assert!(terms.prerequisites.is_empty());
            // At least one credit is charged on the first tick, so the smoke check can see it.
            assert!(terms.cost / i64::from(terms.build_time) > 0 || terms.build_time == 1);
            assert_eq!(first_queueable(definitions, &[], state), None);
            // An empty roster offers nothing.
            let mut nothing = Players::default();
            nothing
                .add_with_roster(sim::PlayerId(0), 0, Default::default())
                .expect("player");
            let state = nothing.get(sim::PlayerId(0)).expect("player");
            assert_eq!(first_queueable(definitions, &owned, state), None);
        }
    }

    #[test]
    fn a_run_passes_only_with_the_fog_computed_and_unseen_enemies_hidden() {
        assert_eq!(
            smoke_failures(&SmokeOutcome {
                visible_cells: 0,
                ..PASSING
            }),
            ["the human player sees no cell after 198 ticks"]
        );
        // Before the first tick nothing is visible yet.
        assert!(
            smoke_failures(&SmokeOutcome {
                ticks: 0,
                visible_cells: 0,
                ..PASSING
            })
            .is_empty()
        );
        assert_eq!(
            smoke_failures(&SmokeOutcome {
                view_mismatches: 3,
                ..PASSING
            }),
            ["3 views are shown or hidden against what the human player sees"]
        );
    }

    #[test]
    fn the_demo_match_expects_all_of_its_starting_units() {
        // M6: the expected starting units and buildings are those of both factions of the menu's
        // choice (the skirmish file, D40), and match setup spawns exactly them.
        // RA1: the factions are countries. As the menu starts the match, each player has the
        // base unit of its side and the starting units of the lobby's unit count
        // (`sim::starting_units`) and no building; with the lobby's "start with an MCV" off,
        // the yard instead of the base unit.
        let catalogue = crate::skirmish::tests::production_catalogue();
        // (RA1-C: with a fixed country for the computer player, `MenuSelection::versus`, which
        // is what `menu_host` and `demo_host` play; the menu itself opens with a RANDOM one.)
        let selection = MenuSelection::versus(&catalogue);
        let (units, buildings) =
            crate::skirmish::selection_totals(&catalogue, selection).expect("fixed countries");
        let factions = &catalogue.skirmish.factions;
        let players = selection.players();
        let rules = catalogue.definitions.rules().expect("rules");
        let unit_count = u32::try_from(rules.dialog.unit_count).expect("unit count");
        let expected: usize = players
            .iter()
            .map(|setup| {
                let faction = &factions[setup.faction];
                let groups: u32 = faction.units.iter().map(|group| group.count).sum();
                groups as usize
                    + sim::starting_units(
                        &catalogue.skirmish,
                        &catalogue.definitions,
                        faction.side.expect("a country"),
                        unit_count,
                        &mut sim::SimRng::new(0),
                    )
                    .len()
            })
            .sum();
        assert!(expected > 0);
        assert_eq!(units, expected + players.len(), "a base unit each");
        assert_eq!(
            buildings,
            players
                .iter()
                .map(|setup| factions[setup.faction].buildings.len())
                .sum::<usize>()
        );
        assert!(units > 0 && buildings == 0);
        let host = crate::sim_host::tests::menu_host();
        let world = host.world();
        let mut query = world.try_query::<(&SimId, &Owner)>().expect("registered");
        assert_eq!(query.iter(world).count(), units + buildings);
        // Before the first tick the human player owns its base unit and no building.
        let base = |kind: sim::UnitKind| rules.is_base_unit(kind);
        let mut own = world.try_query::<(&Owner, &sim::Unit)>().expect("units");
        let bases = own
            .iter(world)
            .filter(|(owner, unit)| owner.0 == HUMAN && base(unit.kind));
        assert_eq!(bases.count(), 1);
        let mut built = world.try_query::<&Building>().expect("registered");
        assert_eq!(built.iter(world).count(), 0);
        // With the yards: a building each in place of the base unit.
        let (yard_units, yards) = crate::skirmish::selection_totals(
            &catalogue,
            MenuSelection {
                mcv_start: false,
                ..selection
            },
        )
        .expect("fixed countries");
        assert_eq!((yard_units, yards), (expected, players.len()));
        assert!(yard_units > 0 && yards > 0);
        let host = crate::sim_host::tests::demo_host();
        let world = host.world();
        let mut query = world.try_query::<(&SimId, &Owner)>().expect("registered");
        assert_eq!(query.iter(world).count(), yard_units + yards);
    }

    #[test]
    fn a_run_passes_only_with_the_menu_the_game_flow_and_the_computer_player() {
        let fails = |outcome: SmokeOutcome| smoke_failures(&outcome);
        assert_eq!(
            fails(SmokeOutcome {
                menu_buttons: 0,
                ..PASSING
            }),
            [format!("the menu showed 0 of {MENU_BUTTONS} buttons")]
        );
        assert_eq!(
            fails(SmokeOutcome {
                flow_done: false,
                starts: 2,
                ..PASSING
            }),
            [format!(
                "2 of {LONG_RUN_STARTS} matches started, the game flow did not reach the last one"
            )]
        );
        assert_eq!(
            fails(SmokeOutcome {
                starts: 1,
                ..PASSING
            })
            .len(),
            1
        );
        // A short run starts one match.
        assert!(
            fails(SmokeOutcome {
                frames: 100,
                starts: 1,
                expected_starts: 1,
                pause: Some((0, true)),
                ..PASSING
            })
            .is_empty()
        );
        // Views of an earlier match left behind, or missing.
        assert_eq!(
            fails(SmokeOutcome {
                views: (27, 2),
                ..PASSING
            }),
            ["27 unit and 2 building views for 26 units and 2 buildings"]
        );
        assert_eq!(
            fails(SmokeOutcome {
                views: (26, 1),
                ..PASSING
            })
            .len(),
            1
        );
        assert_eq!(
            fails(SmokeOutcome {
                ai_commands: 0,
                ..PASSING
            }),
            ["the computer player issued no command in 198 ticks"]
        );
        // Too few ticks to require the computer player's commands.
        assert!(
            fails(SmokeOutcome {
                ticks: AI_CHECK_TICKS - 1,
                ai_commands: 0,
                ..PASSING
            })
            .is_empty()
        );
        // M6 review 1 P2-4: in the reviewer's run with the computer player's start outside the
        // map, all but one of its commands were rejected and the run passed, because only pushed
        // commands were counted. Any rejection of its commands now fails the run, however many
        // it pushed.
        assert_eq!(
            fails(SmokeOutcome {
                ai_commands: 10,
                ai_rejected: 9,
                ..PASSING
            }),
            ["9 commands of the computer player were rejected"]
        );
        assert_eq!(
            fails(SmokeOutcome {
                ai_rejected: 1,
                ..PASSING
            }),
            ["1 commands of the computer player were rejected"]
        );
    }

    #[test]
    fn a_run_passes_only_with_the_full_interface_loaded() {
        let fails = |outcome: SmokeOutcome| smoke_failures(&outcome);
        assert_eq!(
            fails(SmokeOutcome {
                sidebar_items: 8,
                ..PASSING
            }),
            ["the sidebar has 8 of 9 production buttons"]
        );
        assert_eq!(
            fails(SmokeOutcome {
                sidebar_items: 0,
                expected_items: 0,
                ..PASSING
            }),
            ["the sidebar has 0 of 0 production buttons"]
        );
        assert_eq!(
            fails(SmokeOutcome {
                buttons_outside: 2,
                ..PASSING
            }),
            ["2 sidebar buttons lie outside the window"]
        );
        assert_eq!(
            fails(SmokeOutcome {
                credits_text_shown: false,
                ..PASSING
            }),
            ["the sidebar does not show the player's credits"]
        );
        assert_eq!(
            fails(SmokeOutcome {
                minimap_drawings: 0,
                ..PASSING
            }),
            ["the minimap was drawn 0 times with 1000 lit pixels"]
        );
        assert_eq!(
            fails(SmokeOutcome {
                minimap_lit_pixels: 0,
                ..PASSING
            })
            .len(),
            1
        );
        // Before the first tick the minimap need not be drawn.
        assert!(
            fails(SmokeOutcome {
                ticks: 0,
                visible_cells: 0,
                minimap_drawings: 0,
                ..PASSING
            })
            .is_empty()
        );
        // RA2: a map drawn without radar, or a blank screen with it.
        assert_eq!(
            fails(SmokeOutcome {
                minimap_follows_radar: false,
                ..PASSING
            }),
            ["the minimap does not follow the player's radar"]
        );
        assert_eq!(
            fails(SmokeOutcome {
                sounds_complete: false,
                ..PASSING
            }),
            ["not every sound effect was synthesised"]
        );
        assert_eq!(
            fails(SmokeOutcome {
                music_players: 0,
                ..PASSING
            }),
            ["0 entities play the music, expected 1"]
        );
    }

    #[test]
    fn an_exit_before_the_last_frame_fails_the_run() {
        assert!(exits_early(0, 600));
        assert!(exits_early(599, 600));
        assert!(!exits_early(600, 600));
        // An exit message written in the last frame, before or after the checks, fails the run
        // only if it came early.
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_message::<AppExit>();
        app.insert_resource(SmokeTest {
            rendered: 3,
            ..SmokeTest::new(10)
        });
        app.world_mut().write_message(AppExit::Success);
        app.world_mut()
            .run_system_once(fail_on_early_exit)
            .expect("runs");
        assert_eq!(app.should_exit(), Some(AppExit::error()));
    }

    #[test]
    fn a_run_passes_only_if_the_pause_stops_the_ticks_and_refuses_commands() {
        let fails = |pause| smoke_failures(&SmokeOutcome { pause, ..PASSING });
        assert!(fails(Some((0, true))).is_empty());
        assert_eq!(
            fails(Some((3, true))),
            ["3 ticks ran while the game was paused"]
        );
        assert_eq!(
            fails(Some((0, false))),
            ["a command was accepted while the game was paused"]
        );
        assert_eq!(fails(None), ["the pause did not end within the run"]);
    }

    // --- RA1: the lobby's options in the game flow ----------------------------------------------

    #[test]
    fn a_run_passes_only_with_the_menu_laid_out_the_cards_listed_and_the_flow_checked() {
        let fails = |outcome: SmokeOutcome| smoke_failures(&outcome);
        assert_eq!(
            fails(SmokeOutcome {
                unlisted_shown: 2,
                ..PASSING
            }),
            ["2 sidebar cards are shown of items that no building of the player could build"]
        );
        assert_eq!(
            fails(SmokeOutcome {
                cards_missing: true,
                ..PASSING
            }),
            ["the player owns a building and the sidebar shows no card"]
        );
        assert_eq!(
            fails(SmokeOutcome {
                layout_problems: None,
                ..PASSING
            }),
            ["the layout of the menu was not checked"]
        );
        assert_eq!(
            fails(SmokeOutcome {
                layout_problems: Some(3),
                ..PASSING
            }),
            ["the layout of the menu has 3 problems (logged above)"]
        );
        assert_eq!(
            fails(SmokeOutcome {
                flow_problems: 1,
                ..PASSING
            }),
            ["1 checks of the game flow failed (logged above)"]
        );
        // A problem of the flow is counted and logged.
        let mut smoke = SmokeTest::new(600);
        smoke.problem("something");
        smoke.problem(format!("something {}", "else"));
        assert_eq!(smoke.flow_problems, 2);
        assert_eq!(
            MENU_BUTTONS, 73,
            "16 rows, 10 countries, 9 difficulties, 3 buttons"
        );
    }

    #[test]
    fn the_layout_check_finds_a_menu_that_is_too_large_overlaps_or_cuts_a_text() {
        let size = menu_size();
        let window = Vec2::new(1280.0, 800.0);
        let at = |x: f32, y: f32, w: f32, h: f32| Rect::new(x, y, x + w, y + h);
        // A menu of the size of its measures, centred, with two buttons side by side and a text
        // inside its place.
        let origin = (window - size) / 2.0;
        let good = MenuLayout {
            window,
            blocks: vec![at(origin.x, origin.y, size.x, size.y)],
            buttons: vec![
                at(origin.x + 10.0, origin.y + 10.0, 24.0, 26.0),
                at(origin.x + 34.0, origin.y + 10.0, 24.0, 26.0),
            ],
            texts: vec![(
                at(origin.x + 70.0, origin.y + 14.0, 90.0, 17.0),
                at(origin.x + 60.0, origin.y + 10.0, 150.0, 26.0),
            )],
        };
        assert_eq!(good.problems(2), Vec::<String>::new());
        assert_eq!(good.extent().map(|extent| extent.size()), Some(size));
        assert_eq!(
            MenuLayout::default().problems(0),
            ["the menu is not laid out"]
        );
        // A button missing.
        assert_eq!(good.problems(3), ["2 of 3 menu buttons are laid out"]);
        // Two buttons that overlap by a few pixels; buttons that only touch do not.
        let mut overlapping = good.clone();
        overlapping.buttons[1] = at(origin.x + 30.0, origin.y + 10.0, 24.0, 26.0);
        assert_eq!(overlapping.problems(2), ["1 pairs of menu buttons overlap"]);
        // A text wider than its place, and one pushed below it.
        for text in [
            at(origin.x + 55.0, origin.y + 14.0, 170.0, 17.0),
            at(origin.x + 70.0, origin.y + 30.0, 90.0, 17.0),
        ] {
            let mut cut = good.clone();
            cut.texts[0].0 = text;
            assert_eq!(cut.problems(2), ["1 menu texts do not fit their place"]);
        }
        // A menu larger than its measures say: a row grew.
        let mut grown = good.clone();
        grown
            .blocks
            .push(at(origin.x, origin.y, size.x, size.y + 30.0));
        let problems = grown.problems(2);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("more than the"), "{problems:?}");
        // A menu that does not fit the smallest window, and so lies outside this small one.
        let small = Vec2::new(1024.0, 600.0);
        let huge = MenuLayout {
            window: small,
            blocks: vec![at(-150.0, -80.0, 1324.0, 760.0)],
            buttons: Vec::new(),
            texts: Vec::new(),
        };
        let problems = huge.problems(0);
        assert_eq!(problems.len(), 3, "{problems:?}");
        assert!(problems[1].contains("does not fit a window of 1280 x 720"));
        assert!(problems[2].contains("outside the window of 1024 x 600"));
    }

    #[test]
    fn the_flows_menu_presses_choose_its_matches_and_restore_the_lobby() {
        // The choices of a long run, made by the menu's pure functions as its buttons make
        // them: without a window, so that a change of the menu that breaks the flow fails here.
        let catalogue = crate::skirmish::tests::production_catalogue();
        let counts = MenuCounts::of(&catalogue);
        let lobby = counts.lobby;
        let opening = MenuSelection::opening(&catalogue);
        let mut smoke = SmokeTest::new(600);
        // The fullest menu is a menu the buttons could make, with ten players.
        let fullest = fullest_menu(&catalogue, opening);
        assert!(fullest.valid(counts));
        assert_eq!(fullest.players().len(), sim::MAX_MATCH_PLAYERS);
        assert_eq!(
            crate::skirmish::row_text(Row::Map, fullest, &catalogue),
            "continental divide  256 x 256"
        );
        assert_eq!(
            crate::skirmish::row_text(Row::Country(9), fullest, &catalogue),
            "Great Britain (Allied)"
        );
        assert_eq!(
            crate::skirmish::row_text(Row::UnitSpeed, fullest, &catalogue),
            "4x - SUPER ME OFF"
        );

        // The first match: every option of the lobby away from its default.
        let mut first = opening;
        choose_first(&mut smoke, &mut first, &catalogue, counts);
        assert_eq!(smoke.flow_problems, 0);
        assert!(first.valid(counts));
        let map = growth_map(&catalogue);
        assert_eq!(catalogue.maps[map].id, "riverlands");
        assert_eq!(first.map, map);
        assert_eq!(first.enemy_count(), grown_enemies(&catalogue));
        assert_eq!(
            first.players().len(),
            5,
            "more than the four starts of the file"
        );
        let soviet = &catalogue.skirmish.factions[first.faction()];
        assert_eq!(soviet.side, Some(sim::Side::Soviet));
        let options = first.options();
        assert_eq!(
            options,
            sim::MatchOptions {
                super_players: vec![HUMAN],
                unlock_players: vec![HUMAN],
                unit_speed_percent: crate::skirmish::UNIT_SPEEDS[1],
                free_for_all: true,
                // The first two computer players have a country; the others are drawn.
                random_countries: vec![sim::PlayerId(3), sim::PlayerId(4)],
                starting_credits: Some(lobby.min_credits),
                unit_count: Some(lobby.min_unit_count),
                mcv_start: true,
                short_game: false,
                superweapons: false,
                crates: false,
                mcv_repacks: true,
                ore_grows: true,
            }
        );
        assert_eq!(
            (lobby.min_credits, lobby.min_unit_count),
            (5000, 0),
            "the lowest of the lobby of Red Alert 2"
        );
        // The match it starts holds exactly that; another choice does not pass for it.
        let (host, start) = crate::sim_host::tests::match_host(&catalogue, first, SMOKE_SEED);
        let played = MatchPlayers::of(&host, &catalogue, &first.players());
        let spawned = (start.units, start.buildings);
        assert_eq!(spawned, (5, 0), "an MCV each and nothing else");
        let colours = Some((first.color, 5));
        assert_eq!(
            match_problems(first, &catalogue, &host, &played, spawned, colours),
            Vec::<String>::new()
        );
        let filed = &catalogue.maps[map].map.grid;
        assert!(host.grid().expect("grid").width() > filed.width());
        for (other, problem) in [
            (
                MenuSelection {
                    short_game: true,
                    ..first
                },
                "short game, superweapons, MCV repacks or crates are not as chosen",
            ),
            (
                MenuSelection {
                    free_for_all: false,
                    ..first
                },
                "the teams are not as chosen",
            ),
            (
                MenuSelection {
                    credits: Some(lobby.max_credits),
                    ..first
                },
                "the human player did not start with the chosen credits",
            ),
            (
                MenuSelection {
                    unit_count: Some(lobby.max_unit_count),
                    ..first
                },
                "the players did not start with the units and buildings of the unit count and \
                 MCV start",
            ),
            (
                MenuSelection {
                    mcv_start: false,
                    ..first
                },
                "the players did not start with the units and buildings of the unit count and \
                 MCV start",
            ),
            (
                MenuSelection {
                    map_growth: false,
                    ..first
                },
                "the map does not have the size the menu showed",
            ),
            (
                MenuSelection {
                    super_me: false,
                    ..first
                },
                "Super Me, its unit speed or ALL UNLOCKED is not as chosen for the human player",
            ),
            (
                MenuSelection {
                    enemies: first.enemies - 1,
                    ..first
                },
                "the match has other players than the menu chose",
            ),
            (
                first.with_countries(0, 0, counts.factions),
                "a player does not play the country chosen for it",
            ),
        ] {
            let problems = match_problems(other, &catalogue, &host, &played, spawned, colours);
            assert!(
                problems.iter().any(|found| found == problem),
                "{problems:?} lacks {problem:?}"
            );
        }
        assert_eq!(
            match_problems(
                first,
                &catalogue,
                &host,
                &played,
                spawned,
                Some((first.color, 4))
            ),
            ["the players' colours are not the chosen one and one of their own for each"]
        );

        // The match with the yards: as many players as the file has starts, without growth.
        let mut yards = first;
        choose_yards(&mut smoke, &mut yards, &catalogue, counts);
        assert_eq!(smoke.flow_problems, 0);
        assert!(yards.valid(counts));
        assert_eq!(yards.players().len(), 4);
        let (host, start) = crate::sim_host::tests::match_host(&catalogue, yards, SMOKE_SEED);
        let played = MatchPlayers::of(&host, &catalogue, &yards.players());
        assert_eq!(start.buildings, 4, "a Construction Yard each");
        assert_eq!(
            match_problems(
                yards,
                &catalogue,
                &host,
                &played,
                (start.units, start.buildings),
                Some((yards.color, 4))
            ),
            Vec::<String>::new()
        );
        let (bases, own_yards, others) = human_base(&host);
        assert_eq!((bases.len(), own_yards.len(), others), (0, 1, false));
        assert!(cards_of_own_side(&host, Some(&played)));

        // The last match: the lobby's defaults again, on the opening map.
        let mut last = yards;
        choose_last(&mut smoke, &mut last, counts, opening);
        assert_eq!(smoke.flow_problems, 0);
        assert!(last.valid(counts));
        assert_eq!(last.map, opening.map);
        assert_eq!(last.enemy_count(), SMOKE_ENEMIES);
        assert_eq!(last.countries[0], Some(1), "the second country");
        assert_eq!(
            &last.countries[1..],
            &opening.countries[1..],
            "RANDOM again"
        );
        assert_eq!(last.difficulties, opening.difficulties);
        assert_eq!(
            last.options(),
            sim::MatchOptions {
                random_countries: (1..=SMOKE_ENEMIES as u8).map(sim::PlayerId).collect(),
                starting_credits: Some(lobby.credits),
                unit_count: Some(lobby.unit_count),
                unit_speed_percent: 100,
                ..sim::MatchOptions::default()
            }
        );
        // A menu whose buttons do not do what the flow expects is a problem of the flow.
        let mut broken = opening;
        choose_first(
            &mut smoke,
            &mut broken,
            &catalogue,
            MenuCounts { maps: 0, ..counts },
        );
        assert!(smoke.flow_problems > 0);
    }
}
