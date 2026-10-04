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

use std::collections::BTreeMap;

use bevy::asset::UntypedAssetLoadFailedEvent;
use bevy::prelude::*;
use sim::{
    Building, BuildingKind, CellPos, Command, Definitions, Mobile, Owner, Players, Position,
    ProductionItem, SimEvent, SimId, SubPos,
};

use crate::audio::{MusicPlayer, Sfx, SoundBank};
use crate::buildings::{BuildingStatus, BuildingView, STARTING_BUILDINGS, building_shown};
use crate::construction::{InterfaceMode, production_menu};
use crate::map::MapStatus;
use crate::minimap::MinimapState;
use crate::sidebar::{
    SidebarButton, SidebarEffect, SidebarText, button_effect, credits_text, item_status,
};
use crate::sim_host::{CommandRefused, CommandSink, SimHost};
use crate::units::{HUMAN, UnitStatus, UnitView, starting_unit_total, unit_shown};

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
            pause: PauseCheck::default(),
        })
        .add_systems(
            Update,
            (order_move, order_production, watch_events, check_pause).chain(),
        )
        .add_systems(Last, (fail_on_asset_load_error, count_frames).chain());
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
    pause: PauseCheck,
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
    if smoke.frames < PAUSE_MIN_FRAMES {
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
/// the player queue: it has no prerequisites and the player owns a building of its producing
/// type.
pub fn first_queueable(
    definitions: &Definitions,
    owned: &[BuildingKind],
) -> Option<ProductionItem> {
    production_menu(definitions).into_iter().find(|&item| {
        matches!(item, ProductionItem::Building(_))
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
    if smoke.production.is_some() || status.spawned == 0 {
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
    let item = first_queueable(definitions, &owned);
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
    if smoke.order.is_some() || status.spawned == 0 {
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
        if let SimEvent::CommandRejected { reason, .. } = event {
            error!("smoke test failed: a command of the smoke test was rejected: {reason}");
            smoke.rejected = true;
        }
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
    mut exit: MessageWriter<AppExit>,
) {
    smoke.rendered += 1;
    if smoke.rendered < smoke.frames {
        return;
    }
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
    info!(
        "smoke test: {} frames, {} simulation ticks, map created: {}, units spawned: {}, \
         units moved: {moved} in {ticks_since_order} ticks after the order, buildings spawned: \
         {}, production charged {charged} credits in {ticks_since_production} ticks, visible \
         cells: {visible_cells}, enemy views shown: {enemies_shown}, view mismatches: \
         {mismatches}, sidebar items: {sidebar_items} of {expected_items}, sidebar buttons outside the \
         window: {buttons_outside}, sidebar credits: \
         {credits_shown:?}, minimap drawings: {}, minimap pixels lit: {}, sounds: {}, music \
         players: {}, pause: {:?}",
        smoke.rendered,
        tick,
        map.map_created,
        status.spawned,
        buildings.spawned,
        minimap.drawings,
        minimap.lit_pixels,
        sounds,
        music.iter().count(),
        smoke.pause
    );
    let failures = smoke_failures(&SmokeOutcome {
        frames: smoke.frames,
        map_created: map.map_created,
        spawned: status.spawned,
        expected_units: starting_unit_total(),
        rejected: smoke.rejected,
        ticks_since_order,
        moved,
        buildings: buildings.spawned,
        expected_buildings: STARTING_BUILDINGS.len(),
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
    };

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
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let definitions = sim::load_definitions(
            root.join(crate::units::UNITS_PATH),
            root.join(crate::buildings::ECONOMY_PATH),
        )
        .unwrap_or_else(|error| panic!("{error}"));
        let owned: Vec<BuildingKind> = STARTING_BUILDINGS
            .iter()
            .filter(|(owner, ..)| *owner == HUMAN)
            .filter_map(|(_, building_type, _)| definitions.building_kind(building_type))
            .collect();
        let item = first_queueable(&definitions, &owned).expect("something to queue");
        let terms = definitions.production(item).expect("terms");
        assert!(terms.prerequisites.is_empty());
        // At least one credit is charged on the first tick, so the smoke check can see it.
        assert!(terms.cost / i64::from(terms.build_time) > 0 || terms.build_time == 1);
        assert_eq!(first_queueable(&definitions, &[]), None);
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
        assert_eq!(starting_unit_total(), 12 + 6 + 5 + 3);
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
