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
//! From M7 the last match of a long run is started after the faction row's `>` button, so the
//! human player plays the second faction; no sidebar button of an item outside the roster of the
//! human player's faction may be displayed; the menu has its Options button, and the options
//! screen all of its buttons; and the run changes two options the options screen's way
//! (`crate::options::options_effect`), which must be written to the run's own settings file
//! ([`settings_path`], removed at the end) and read back unchanged (docs/DECISIONS.md D52).

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
    Catalogue, EndButton, EndEffect, LastMatch, MenuButton, MenuCounts, MenuEffect, MenuRoot,
    MenuSelection, Row, Screen, begin_match, end_effect, end_match, menu_effect,
};
use crate::units::{HUMAN, OPPONENT, UnitStatus, UnitView, unit_shown};

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
        app.insert_resource(SmokeTest {
            frames: self.frames,
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
        })
        .add_systems(
            Update,
            (
                play_flow,
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
}

/// The game flow the smoke test plays (M6).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Flow {
    /// In the menu; the first match starts.
    #[default]
    Menu,
    /// The first match runs; it is restarted after [`FLOW_TICKS`] ticks.
    First,
    /// The restarted match runs; the game returns to the menu after [`FLOW_TICKS`] ticks.
    Restarted,
    /// Back in the menu; the last match starts.
    MenuAgain,
    /// The last match runs, and the checks of M1 to M5 apply to it.
    Last,
}

/// Plays the game flow: starts, restarts, leaves, and starts the match again, as the menu's and
/// the end screen's buttons do (their effects are computed by the same pure functions).
#[allow(clippy::too_many_arguments)]
fn play_flow(
    mut commands: Commands,
    mut smoke: ResMut<SmokeTest>,
    screen: Res<Screen>,
    catalogue: Option<Res<Catalogue>>,
    mut selection: ResMut<MenuSelection>,
    last: Res<LastMatch>,
    map: Res<MapStatus>,
    host: Res<SimHost>,
    menu: Query<&MenuButton>,
    roots: Query<&Node, With<MenuRoot>>,
) {
    let Some(catalogue) = catalogue else {
        return;
    };
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
        };
    let settled = map.map_created && host.tick() >= FLOW_TICKS;
    match (smoke.flow, *screen) {
        (Flow::Menu, Screen::Menu) => {
            let shown = roots.iter().any(|node| node.display != Display::None);
            smoke.menu_buttons = Some(if shown { menu.iter().count() } else { 0 });
            start(
                &mut smoke,
                &mut commands,
                *selection,
                if long { Flow::First } else { Flow::Last },
            );
        }
        (Flow::First, Screen::Match) if settled => {
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
            smoke.flow = Flow::Restarted;
        }
        (Flow::Restarted, Screen::Match) if settled => {
            if end_effect(EndButton::Menu, last.0) == EndEffect::Menu {
                commands.queue(end_match);
            }
            smoke.flow = Flow::MenuAgain;
        }
        (Flow::MenuAgain, Screen::Menu) => {
            // M7: the last match is started after the faction row's `>` button, so that the human
            // player plays the next faction (the second, with the shipped data) and the checks
            // cover it.
            match menu_effect(MenuButton::Next(Row::Faction), *selection, counts) {
                MenuEffect::Select(next) => *selection = next,
                effect => {
                    error!("smoke test failed: the faction row's next button did {effect:?}");
                    smoke.rejected = true;
                }
            }
            start(&mut smoke, &mut commands, *selection, Flow::Last);
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
fn check_pause(mut smoke: ResMut<SmokeTest>, mut sink: ResMut<CommandSink>, host: Res<SimHost>) {
    if smoke.frames < PAUSE_MIN_FRAMES || smoke.flow != Flow::Last {
        return;
    }
    let frame = smoke.rendered;
    match smoke.pause {
        PauseCheck::NotYet if frame >= smoke.frames / 2 => {
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
/// the player with `state` queue: the roster of its faction offers it (M7), it has no
/// prerequisites, and the player owns a building of its producing type.
pub fn first_queueable(
    definitions: &Definitions,
    owned: &[BuildingKind],
    state: &PlayerState,
) -> Option<ProductionItem> {
    production_menu(definitions).into_iter().find(|&item| {
        matches!(item, ProductionItem::Building(_))
            && state.offers(item)
            && definitions.production(item).is_some_and(|terms| {
                terms.prerequisites.is_empty() && owned.contains(&terms.producer)
            })
    })
}

/// Queues the first building the player can produce once its starting buildings exist, as a
/// left click on its sidebar button does.
fn order_production(
    mut smoke: ResMut<SmokeTest>,
    status: Res<BuildingStatus>,
    host: Res<SimHost>,
    mut sink: ResMut<CommandSink>,
) {
    if smoke.production.is_some() || status.spawned == 0 || smoke.flow != Flow::Last {
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
        let item_state = item_status(item, definitions, &owned.iter().copied().collect(), state);
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
/// all of them selected does.
fn order_move(
    mut smoke: ResMut<SmokeTest>,
    status: Res<UnitStatus>,
    host: Res<SimHost>,
    mut sink: ResMut<CommandSink>,
) {
    if smoke.order.is_some() || status.spawned == 0 || smoke.flow != Flow::Last {
        return;
    }
    let start: BTreeMap<SimId, SubPos> = host
        .world()
        .try_query::<(&SimId, &Owner, &Position)>()
        .map(|mut units| {
            units
                .iter(host.world())
                .filter(|(_, owner, _)| owner.0 == HUMAN)
                .map(|(id, _, position)| (*id, position.0))
                .collect()
        })
        .unwrap_or_default();
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
            } else if *player == OPPONENT {
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
    flow: (Option<Res<Catalogue>>, Res<LastMatch>),
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
    let (catalogue, last) = flow;
    let (expected_units, expected_buildings) = match (catalogue.as_deref(), last.0) {
        (Some(catalogue), Some(selection)) => starting_totals(catalogue, selection),
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
    let ai_commands = host
        .ai()
        .and_then(|ai| ai.controller(OPPONENT))
        .map_or(0, |controller| controller.commands_issued());
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
         roster shown: {off_roster_shown}, options buttons: {options_buttons}, settings saved: \
         {settings_saved}",
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
        smoke.ai_rejected
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
        production_ordered: production_ordered || smoke.production.is_none(),
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
            3
        } else {
            1
        },
        views: (unit_view_count, building_view_count),
        simulated: (sim_units, sim_buildings),
        ai_commands,
        ai_rejected: smoke.ai_rejected,
        off_roster_shown,
        options_buttons,
        settings_saved,
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
    /// `false` if the player's starting buildings let it queue nothing.
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
    /// Buttons of the options screen (M7).
    options_buttons: usize,
    /// The options the run changed were written and read back unchanged (M7).
    settings_saved: bool,
}

/// Buttons of the menu: previous and next for each of its rows, Start, Options (M7), and Quit.
pub const MENU_BUTTONS: usize = crate::skirmish::Row::ALL.len() * 2 + 3;

/// What the options screen and the settings file show, for the smoke test's M7 checks.
type OptionQueries<'w, 's> = (
    Res<'w, SettingsFile>,
    Res<'w, VolumeSettings>,
    Res<'w, CameraOptions>,
    Query<'w, 's, &'static OptionsButton>,
);

/// The starting units and buildings of a match with `selection`: those of both players'
/// factions (the skirmish file, M6).
pub fn starting_totals(catalogue: &Catalogue, selection: MenuSelection) -> (usize, usize) {
    selection
        .players()
        .iter()
        .filter_map(|setup| catalogue.skirmish.factions.get(setup.faction))
        .fold((0, 0), |(units, buildings), faction| {
            (
                units
                    + faction
                        .units
                        .iter()
                        .map(|group| group.count as usize)
                        .sum::<usize>(),
                buildings + faction.buildings.len(),
            )
        })
}

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
        failures.push("the player's starting buildings let it queue nothing".to_string());
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
        sounds_complete: true,
        music_players: 1,
        pause: Some((0, true)),
        menu_buttons: MENU_BUTTONS,
        flow_done: true,
        starts: 3,
        expected_starts: 3,
        views: (26, 2),
        simulated: (26, 2),
        ai_commands: 4,
        ai_rejected: 0,
        off_roster_shown: 0,
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
            ["the player's starting buildings let it queue nothing"]
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
        let catalogue = crate::skirmish::tests::production_catalogue();
        let selection = MenuSelection::initial(&catalogue);
        let definitions = &catalogue.definitions;
        assert!(selection.faction < catalogue.skirmish.factions.len());
        for (index, faction) in catalogue.skirmish.factions.iter().enumerate() {
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
        let catalogue = crate::skirmish::tests::production_catalogue();
        let selection = MenuSelection::initial(&catalogue);
        let (units, buildings) = starting_totals(&catalogue, selection);
        let factions = &catalogue.skirmish.factions;
        let players = selection.players();
        let expected: u32 = players
            .iter()
            .flat_map(|setup| &factions[setup.faction].units)
            .map(|group| group.count)
            .sum();
        assert_eq!(units, expected as usize);
        assert_eq!(
            buildings,
            players
                .iter()
                .map(|setup| factions[setup.faction].buildings.len())
                .sum::<usize>()
        );
        assert!(units > 0 && buildings > 0);
        let host = crate::sim_host::tests::demo_host();
        let world = host.world();
        let mut query = world.try_query::<(&SimId, &Owner)>().expect("registered");
        assert_eq!(query.iter(world).count(), units + buildings);
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
            ["2 of 3 matches started, the game flow did not reach the last one"]
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
            frames: 10,
            rendered: 3,
            order: None,
            production: None,
            rejected: false,
            ai_rejected: 0,
            pause: PauseCheck::default(),
            flow: Flow::default(),
            menu_buttons: None,
            starts: 0,
            options_changed: false,
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
}
