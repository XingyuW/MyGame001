//! The sidebar (PLAN.md section 7, M5; docs/DECISIONS.md D35): the minimap, the human player's
//! credits and power, build tabs with a button per production item showing its cost, queue, and
//! progress, buttons for selling, rally points, and pause, and the volume settings.
//!
//! | Input | Effect |
//! |-------|--------|
//! | Left click on an item | `QueueProduction` of the item (if its prerequisites and producer are owned); on a building that is ready, placement mode (`crate::construction`) |
//! | Right click on an item | `CancelProduction` of the item (if one is queued) |
//! | Tabs, or `Tab` | show the buildings or the units |
//! | Sell, Rally | sell mode, rally mode (`crate::construction`) |
//! | Pause, or `P` | pause or resume (`crate::sim_host::CommandSink`) |
//! | Volume buttons, `-`, `=`, `M` | master, music, and effects volume, mute (`crate::audio`) |
//!
//! The sidebar shows only the human player's own state. Its decisions are pure functions,
//! unit tested below; the systems read the simulation through `SimHost` and change it only by
//! pushing commands into `CommandSink` (invariant I5, D34).

use std::collections::BTreeSet;

use bevy::prelude::*;
use sim::{Building, BuildingKind, Command, Definitions, Owner, PlayerState, ProductionItem};

use crate::audio::{Channel, VOLUME_STEPS, VolumeSettings};
use crate::construction::{InterfaceMode, menu_command, production_menu};
use crate::hotkeys::{Hotkey, pressed_hotkeys};
use crate::minimap::{MINIMAP_SIZE, Minimap, MinimapFrame, MinimapNode};
use crate::sim_host::{CommandSink, SimHost};
use crate::units::HUMAN;

/// Width of the sidebar on the right of the window, in logical pixels.
pub const SIDEBAR_WIDTH: f32 = 240.0;

pub struct SidebarPlugin;

impl Plugin for SidebarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tab>()
            .add_systems(Startup, spawn_sidebar.after(crate::minimap::create_minimap))
            .add_systems(Update, (populate_items, update_sidebar).chain())
            .add_systems(
                PostUpdate,
                sidebar_input
                    .after(crate::map::update_hovered_cell)
                    .before(crate::selection::prune_selection),
            );
    }
}

/// Whether the cursor (logical pixels from the window's top left) is over the sidebar of a
/// window `window_width` pixels wide.
pub fn over_sidebar(cursor: Vec2, window_width: f32) -> bool {
    cursor.x >= window_width - SIDEBAR_WIDTH
}

/// The production tab shown.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Buildings,
    Units,
}

impl Tab {
    /// The tab an item is listed in.
    pub fn of(item: ProductionItem) -> Tab {
        match item {
            ProductionItem::Building(_) => Tab::Buildings,
            ProductionItem::Unit(_) => Tab::Units,
        }
    }

    pub fn next(self) -> Tab {
        match self {
            Tab::Buildings => Tab::Units,
            Tab::Units => Tab::Buildings,
        }
    }
}

/// A button of the sidebar.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarButton {
    Item(ProductionItem),
    Tab(Tab),
    Sell,
    Rally,
    Pause,
    /// Lowers (`false`) or raises (`true`) a volume channel.
    Volume(Channel, bool),
    Mute,
}

/// A text of the sidebar that changes during the match.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarText {
    Credits,
    Power,
    Mode,
    Badge(ProductionItem),
    Volume(Channel),
    PauseLabel,
    MuteLabel,
}

/// The filled part of an item's progress bar.
#[derive(Component, Debug)]
pub struct ItemProgress(pub ProductionItem);

/// The node that holds the item buttons.
#[derive(Component, Debug)]
pub struct ItemList;

/// The "PAUSED" banner over the map.
#[derive(Component, Debug)]
pub struct PauseOverlay;

// --- Pure functions -------------------------------------------------------------------------

/// The state of a production item for the human player.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ItemStatus {
    /// The player owns every prerequisite and a producer, so `QueueProduction` is accepted
    /// (unless the queue is full).
    pub available: bool,
    /// Entries of the item in its queue.
    pub queued: usize,
    /// Progress and build time of the item if it is at the head of its queue.
    pub progress: Option<(u32, u32)>,
    /// A building that is complete and waits for placement.
    pub ready: bool,
}

/// The status of `item` for a player with `state` who owns buildings of the types `owned`.
pub fn item_status(
    item: ProductionItem,
    definitions: &Definitions,
    owned: &BTreeSet<BuildingKind>,
    state: &PlayerState,
) -> ItemStatus {
    let Some(terms) = definitions.production(item) else {
        return ItemStatus::default();
    };
    let available = owned.contains(&terms.producer)
        && terms
            .prerequisites
            .iter()
            .all(|prerequisite| owned.contains(prerequisite));
    let queue = state.queue(terms.producer);
    let queued = queue.iter().filter(|entry| entry.item == item).count();
    let progress = queue
        .first()
        .filter(|head| head.item == item)
        .map(|head| (head.progress, terms.build_time));
    let ready = matches!(item, ProductionItem::Building(_))
        && progress.is_some_and(|(done, total)| done >= total);
    ItemStatus {
        available,
        queued,
        progress,
        ready,
    }
}

/// What a click on a sidebar button does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SidebarEffect {
    Nothing,
    Commands(Vec<Command>),
    Mode(InterfaceMode),
    Tab(Tab),
    TogglePause,
    Volume(Channel, bool),
    ToggleMute,
}

/// The effect of clicking `button` with `mouse` (left or right). `menu` is the production menu
/// and `status` the clicked item's status; `mode` the current interface mode.
pub fn button_effect(
    button: SidebarButton,
    mouse: MouseButton,
    menu: &[ProductionItem],
    status: ItemStatus,
    mode: InterfaceMode,
) -> SidebarEffect {
    let left = mouse == MouseButton::Left;
    let right = mouse == MouseButton::Right;
    match button {
        SidebarButton::Item(item) => {
            let Some(index) = menu.iter().position(|&entry| entry == item) else {
                return SidebarEffect::Nothing;
            };
            if left
                && status.ready
                && let ProductionItem::Building(kind) = item
            {
                return SidebarEffect::Mode(InterfaceMode::Place(kind));
            }
            let command = if left && status.available {
                menu_command(menu, index, false)
            } else if right && status.queued > 0 {
                menu_command(menu, index, true)
            } else {
                None
            };
            command.map_or(SidebarEffect::Nothing, |command| {
                SidebarEffect::Commands(vec![command])
            })
        }
        _ if !left => SidebarEffect::Nothing,
        SidebarButton::Tab(tab) => SidebarEffect::Tab(tab),
        SidebarButton::Sell => SidebarEffect::Mode(if mode == InterfaceMode::Sell {
            InterfaceMode::Normal
        } else {
            InterfaceMode::Sell
        }),
        SidebarButton::Rally => SidebarEffect::Mode(if mode == InterfaceMode::Rally {
            InterfaceMode::Normal
        } else {
            InterfaceMode::Rally
        }),
        SidebarButton::Pause => SidebarEffect::TogglePause,
        SidebarButton::Volume(channel, up) => SidebarEffect::Volume(channel, up),
        SidebarButton::Mute => SidebarEffect::ToggleMute,
    }
}

/// The name of an item and its cost, e.g. `power plant  $300`.
pub fn item_label(item: ProductionItem, definitions: &Definitions) -> String {
    let name = match item {
        ProductionItem::Building(kind) => definitions.building(kind).map(|def| def.id.as_str()),
        ProductionItem::Unit(kind) => definitions.unit(kind).map(|def| def.id.as_str()),
    }
    .unwrap_or("?")
    .replace('_', " ");
    match definitions.production(item) {
        Some(terms) => format!("{name}  ${}", terms.cost),
        None => name,
    }
}

/// The badge of an item: `READY`, its progress in percent, how many are queued, or `locked`.
pub fn badge_text(status: ItemStatus) -> String {
    if status.ready {
        return "READY".into();
    }
    let mut parts = Vec::new();
    if let Some((done, total)) = status.progress {
        let percent = u64::from(done.min(total)) * 100 / u64::from(total.max(1));
        parts.push(format!("{percent}%"));
    }
    let waiting = status
        .queued
        .saturating_sub(usize::from(status.progress.is_some()));
    if waiting > 0 {
        parts.push(format!("+{waiting}"));
    }
    if parts.is_empty() && !status.available {
        parts.push("locked".into());
    }
    parts.join(" ")
}

/// Width of an item's progress bar in percent.
pub fn progress_percent(status: ItemStatus) -> f32 {
    status.progress.map_or(0.0, |(done, total)| {
        (done.min(total) as f32 / total.max(1) as f32) * 100.0
    })
}

pub fn credits_text(state: Option<&PlayerState>) -> String {
    format!("Credits  {}", state.map_or(0, PlayerState::credits))
}

/// The power line, e.g. `Power  40 / 120` (used / produced), `LOW` under low power.
pub fn power_text(state: Option<&PlayerState>) -> String {
    let Some(state) = state else {
        return "Power  0 / 0".into();
    };
    format!(
        "Power  {} / {}{}",
        state.power_consumed(),
        state.power_produced(),
        if state.low_power() { "  LOW" } else { "" }
    )
}

/// The hint for the current mode.
pub fn mode_text(mode: InterfaceMode, paused: bool, definitions: Option<&Definitions>) -> String {
    if paused {
        return "Paused: press P or Pause to resume".into();
    }
    match mode {
        InterfaceMode::Normal => {
            "Left: select   Right: order\nS stop  A attack-move  Ctrl+0-9 group".into()
        }
        InterfaceMode::Place(kind) => {
            let name = definitions
                .and_then(|definitions| definitions.building(kind))
                .map_or("building".to_string(), |def| def.id.replace('_', " "));
            format!("Place {name}: left click on green\nRight click or Esc: cancel")
        }
        InterfaceMode::Sell => "Sell: left click on a building\nRight click or Esc: done".into(),
        InterfaceMode::Rally => {
            "Rally point: left click on a cell\nRight click or Esc: cancel".into()
        }
    }
}

/// The text of a volume channel, e.g. `Music 5`.
pub fn volume_text(volume: &VolumeSettings, channel: Channel) -> String {
    let name = match channel {
        Channel::Master => "Master",
        Channel::Music => "Music",
        Channel::Effects => "Effects",
    };
    format!("{name} {}/{VOLUME_STEPS}", volume.level(channel))
}

/// The background colour of a button.
pub fn button_colour(
    button: SidebarButton,
    status: ItemStatus,
    tab: Tab,
    mode: InterfaceMode,
    paused: bool,
    hovered: bool,
) -> Color {
    let base: (f32, f32, f32) = match button {
        SidebarButton::Item(_) if status.ready => (0.55, 0.45, 0.1),
        SidebarButton::Item(_) if status.progress.is_some() || status.queued > 0 => {
            (0.15, 0.3, 0.5)
        }
        SidebarButton::Item(_) if !status.available => (0.13, 0.13, 0.15),
        SidebarButton::Tab(shown) if shown == tab => (0.3, 0.4, 0.55),
        SidebarButton::Sell if mode == InterfaceMode::Sell => (0.55, 0.2, 0.15),
        SidebarButton::Rally if mode == InterfaceMode::Rally => (0.2, 0.5, 0.25),
        SidebarButton::Pause if paused => (0.6, 0.45, 0.1),
        _ => (0.24, 0.26, 0.3),
    };
    let lift = if hovered { 0.08 } else { 0.0 };
    Color::srgb(base.0 + lift, base.1 + lift, base.2 + lift)
}

// --- Reading the simulation -----------------------------------------------------------------

/// The building types the human player owns.
fn owned_kinds(host: &SimHost) -> BTreeSet<BuildingKind> {
    let Some(mut buildings) = host.world().try_query::<(&Owner, &Building)>() else {
        return BTreeSet::new();
    };
    buildings
        .iter(host.world())
        .filter(|(owner, _)| owner.0 == HUMAN)
        .map(|(_, building)| building.kind)
        .collect()
}

/// The status of every production item for the human player.
fn statuses(host: &SimHost) -> Vec<(ProductionItem, ItemStatus)> {
    let (Some(definitions), Some(state)) = (
        host.definitions(),
        host.players().and_then(|players| players.get(HUMAN)),
    ) else {
        return Vec::new();
    };
    let owned = owned_kinds(host);
    production_menu(definitions)
        .into_iter()
        .map(|item| (item, item_status(item, definitions, &owned, state)))
        .collect()
}

// --- Systems --------------------------------------------------------------------------------

fn text(value: impl Into<String>, size: f32) -> (Text, TextFont, TextColor) {
    (
        Text::new(value),
        TextFont::from_font_size(size),
        TextColor(Color::srgb(0.9, 0.92, 0.95)),
    )
}

fn button(kind: SidebarButton, width: Val, height: f32) -> impl Bundle {
    (
        kind,
        Button,
        Node {
            width,
            height: Val::Px(height),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgb(0.24, 0.26, 0.3)),
    )
}

pub(crate) fn spawn_sidebar(mut commands: Commands, minimap: Option<Res<Minimap>>) {
    let image = minimap
        .map(|minimap| minimap.image.clone())
        .unwrap_or_default();
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Px(SIDEBAR_WIDTH),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                row_gap: Val::Px(5.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.07, 0.08, 0.1, 0.96)),
        ))
        .with_children(|sidebar| {
            sidebar
                .spawn((
                    MinimapNode,
                    ImageNode::new(image),
                    bevy::ui::RelativeCursorPosition::default(),
                    Node {
                        width: Val::Px(MINIMAP_SIZE.x),
                        height: Val::Px(MINIMAP_SIZE.y),
                        ..default()
                    },
                    BackgroundColor(Color::BLACK),
                ))
                .with_children(|minimap| {
                    minimap.spawn((
                        MinimapFrame,
                        Node {
                            position_type: PositionType::Absolute,
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BorderColor::all(Color::WHITE),
                    ));
                });
            sidebar.spawn((SidebarText::Credits, text("Credits", 18.0)));
            sidebar.spawn((SidebarText::Power, text("Power", 14.0)));
            sidebar
                .spawn(Node {
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    for (tab, label) in [(Tab::Buildings, "Buildings"), (Tab::Units, "Units")] {
                        row.spawn(button(SidebarButton::Tab(tab), Val::Percent(50.0), 24.0))
                            .with_child(text(label, 13.0));
                    }
                });
            sidebar.spawn((
                ItemList,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(3.0),
                    ..default()
                },
            ));
            sidebar
                .spawn(Node {
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn(button(SidebarButton::Sell, Val::Percent(33.0), 24.0))
                        .with_child(text("Sell", 13.0));
                    row.spawn(button(SidebarButton::Rally, Val::Percent(33.0), 24.0))
                        .with_child(text("Rally", 13.0));
                    row.spawn(button(SidebarButton::Pause, Val::Percent(33.0), 24.0))
                        .with_child((SidebarText::PauseLabel, text("Pause", 13.0)));
                });
            for channel in [Channel::Master, Channel::Music, Channel::Effects] {
                sidebar
                    .spawn(Node {
                        column_gap: Val::Px(4.0),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn(button(
                            SidebarButton::Volume(channel, false),
                            Val::Px(24.0),
                            20.0,
                        ))
                        .with_child(text("-", 13.0));
                        row.spawn((
                            SidebarText::Volume(channel),
                            text("", 13.0),
                            Node {
                                width: Val::Px(100.0),
                                ..default()
                            },
                        ));
                        row.spawn(button(
                            SidebarButton::Volume(channel, true),
                            Val::Px(24.0),
                            20.0,
                        ))
                        .with_child(text("+", 13.0));
                    });
            }
            sidebar
                .spawn(button(SidebarButton::Mute, Val::Percent(100.0), 20.0))
                .with_child((SidebarText::MuteLabel, text("Mute", 13.0)));
            sidebar.spawn((SidebarText::Mode, text("", 12.0)));
        });
    commands
        .spawn((
            PauseOverlay,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(SIDEBAR_WIDTH),
                top: Val::Percent(40.0),
                justify_content: JustifyContent::Center,
                display: Display::None,
                ..default()
            },
        ))
        .with_child(text("PAUSED", 48.0));
}

/// Adds a button per production item once the match is set up.
fn populate_items(
    mut commands: Commands,
    host: Res<SimHost>,
    lists: Query<(Entity, Option<&Children>), With<ItemList>>,
) {
    let Some(definitions) = host.definitions() else {
        return;
    };
    let menu = production_menu(definitions);
    if menu.is_empty() {
        return;
    }
    for (list, children) in &lists {
        if children.is_some_and(|children| !children.is_empty()) {
            continue;
        }
        commands.entity(list).with_children(|list| {
            for &item in &menu {
                list.spawn((
                    SidebarButton::Item(item),
                    Button,
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(30.0),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::SpaceBetween,
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.24, 0.26, 0.3)),
                ))
                .with_children(|entry| {
                    entry
                        .spawn(Node {
                            justify_content: JustifyContent::SpaceBetween,
                            ..default()
                        })
                        .with_children(|line| {
                            line.spawn(text(item_label(item, definitions), 13.0));
                            line.spawn((SidebarText::Badge(item), text("", 13.0)));
                        });
                    entry
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(3.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.1, 0.1, 0.12)),
                        ))
                        .with_child((
                            ItemProgress(item),
                            Node {
                                width: Val::Percent(0.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.3, 0.85, 0.35)),
                        ));
                });
            }
        });
    }
}

/// Clicks on sidebar buttons and the sidebar's hotkeys.
#[allow(clippy::too_many_arguments)]
fn sidebar_input(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    host: Res<SimHost>,
    pressed: Query<(&Interaction, &SidebarButton), Changed<Interaction>>,
    buttons: Query<(&Interaction, &SidebarButton)>,
    mut tab: ResMut<Tab>,
    mut mode: ResMut<InterfaceMode>,
    mut sink: ResMut<CommandSink>,
    mut volume: ResMut<VolumeSettings>,
) {
    let mut clicks: Vec<(SidebarButton, MouseButton)> = pressed
        .iter()
        .filter(|(interaction, _)| **interaction == Interaction::Pressed)
        .map(|(_, &button)| (button, MouseButton::Left))
        .collect();
    if mouse.just_pressed(MouseButton::Right) {
        clicks.extend(
            buttons
                .iter()
                .filter(|(interaction, _)| **interaction != Interaction::None)
                .map(|(_, &button)| (button, MouseButton::Right)),
        );
    }
    let mut effects = Vec::new();
    if !clicks.is_empty() {
        let menu = host.definitions().map(production_menu).unwrap_or_default();
        let statuses = statuses(&host);
        for (button, mouse) in clicks {
            let status = match button {
                SidebarButton::Item(item) => statuses
                    .iter()
                    .find(|(entry, _)| *entry == item)
                    .map(|(_, status)| *status)
                    .unwrap_or_default(),
                _ => ItemStatus::default(),
            };
            effects.push(button_effect(button, mouse, &menu, status, *mode));
        }
    }
    for key in pressed_hotkeys(&keys) {
        effects.push(match key {
            Hotkey::NextTab => SidebarEffect::Tab(tab.next()),
            Hotkey::Pause => SidebarEffect::TogglePause,
            Hotkey::VolumeDown => SidebarEffect::Volume(Channel::Master, false),
            Hotkey::VolumeUp => SidebarEffect::Volume(Channel::Master, true),
            Hotkey::Mute => SidebarEffect::ToggleMute,
            _ => SidebarEffect::Nothing,
        });
    }
    for effect in effects {
        match effect {
            SidebarEffect::Nothing => {}
            SidebarEffect::Commands(commands) => {
                sink.issue(commands);
            }
            SidebarEffect::Mode(next) => *mode = next,
            SidebarEffect::Tab(next) => *tab = next,
            SidebarEffect::TogglePause => {
                let paused = sink.toggle_pause();
                info!(
                    "{}",
                    if paused {
                        "game paused"
                    } else {
                        "game resumed"
                    }
                );
            }
            SidebarEffect::Volume(channel, up) => volume.step(channel, up),
            SidebarEffect::ToggleMute => volume.muted = !volume.muted,
        }
    }
}

/// Brings the sidebar's texts, colours, progress bars, and the pause banner up to date.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn update_sidebar(
    host: Res<SimHost>,
    sink: Res<CommandSink>,
    tab: Res<Tab>,
    mode: Res<InterfaceMode>,
    volume: Res<VolumeSettings>,
    mut texts: Query<(&mut Text, &mut TextColor, &SidebarText)>,
    mut buttons: Query<
        (
            &SidebarButton,
            &Interaction,
            &mut BackgroundColor,
            &mut Node,
        ),
        (
            Without<ItemProgress>,
            Without<PauseOverlay>,
            Without<MinimapFrame>,
        ),
    >,
    mut progress: Query<
        (&ItemProgress, &mut Node),
        (
            Without<SidebarButton>,
            Without<PauseOverlay>,
            Without<MinimapFrame>,
        ),
    >,
    mut overlay: Query<
        &mut Node,
        (
            With<PauseOverlay>,
            Without<SidebarButton>,
            Without<ItemProgress>,
            Without<MinimapFrame>,
        ),
    >,
) {
    let statuses = statuses(&host);
    let status_of = |item: ProductionItem| {
        statuses
            .iter()
            .find(|(entry, _)| *entry == item)
            .map(|(_, status)| *status)
            .unwrap_or_default()
    };
    let state = host.players().and_then(|players| players.get(HUMAN));
    let paused = sink.paused();
    for (mut text, mut colour, kind) in &mut texts {
        let value = match kind {
            SidebarText::Credits => credits_text(state),
            SidebarText::Power => power_text(state),
            SidebarText::Mode => mode_text(*mode, paused, host.definitions()),
            SidebarText::Badge(item) => badge_text(status_of(*item)),
            SidebarText::Volume(channel) => volume_text(&volume, *channel),
            SidebarText::PauseLabel => if paused { "Resume" } else { "Pause" }.into(),
            SidebarText::MuteLabel => if volume.muted { "Unmute" } else { "Mute" }.into(),
        };
        if text.0 != value {
            text.0 = value;
        }
        let wanted = if *kind == SidebarText::Power && state.is_some_and(PlayerState::low_power) {
            Color::srgb(1.0, 0.4, 0.3)
        } else {
            Color::srgb(0.9, 0.92, 0.95)
        };
        if colour.0 != wanted {
            colour.0 = wanted;
        }
    }
    for (button, interaction, mut background, mut node) in &mut buttons {
        let status = match button {
            SidebarButton::Item(item) => status_of(*item),
            _ => ItemStatus::default(),
        };
        let wanted = button_colour(
            *button,
            status,
            *tab,
            *mode,
            paused,
            *interaction != Interaction::None,
        );
        if background.0 != wanted {
            background.0 = wanted;
        }
        if let SidebarButton::Item(item) = button {
            let display = if Tab::of(*item) == *tab {
                Display::Flex
            } else {
                Display::None
            };
            if node.display != display {
                node.display = display;
            }
        }
    }
    for (bar, mut node) in &mut progress {
        let width = Val::Percent(progress_percent(status_of(bar.0)));
        if node.width != width {
            node.width = width;
        }
    }
    for mut node in &mut overlay {
        let display = if paused { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use sim::{CellPos, UnitKind};

    use super::*;

    fn production_definitions() -> Definitions {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        crate::units::load_match_data(&root)
            .unwrap_or_else(|error| panic!("{error}"))
            .0
            .definitions
    }

    #[test]
    fn the_cursor_is_over_the_sidebar_on_its_whole_width() {
        assert!(!over_sidebar(Vec2::new(1039.9, 10.0), 1280.0));
        assert!(over_sidebar(Vec2::new(1040.0, 10.0), 1280.0));
        assert!(over_sidebar(Vec2::new(1279.0, 700.0), 1280.0));
        // A window narrower than the sidebar is all sidebar.
        assert!(over_sidebar(Vec2::new(0.0, 0.0), 200.0));
    }

    #[test]
    fn item_buttons_queue_cancel_and_start_placement() {
        let definitions = production_definitions();
        let menu = production_menu(&definitions);
        let power = definitions
            .building_kind("power_plant")
            .expect("power plant");
        let item = ProductionItem::Building(power);
        let available = ItemStatus {
            available: true,
            ..ItemStatus::default()
        };
        // Left: queue an available item (QueueProduction).
        assert_eq!(
            button_effect(
                SidebarButton::Item(item),
                MouseButton::Left,
                &menu,
                available,
                InterfaceMode::Normal
            ),
            SidebarEffect::Commands(vec![Command::QueueProduction { item }])
        );
        // Right: cancel a queued item (CancelProduction); nothing if none is queued.
        let queued = ItemStatus {
            queued: 2,
            ..available
        };
        assert_eq!(
            button_effect(
                SidebarButton::Item(item),
                MouseButton::Right,
                &menu,
                queued,
                InterfaceMode::Normal
            ),
            SidebarEffect::Commands(vec![Command::CancelProduction { item }])
        );
        assert_eq!(
            button_effect(
                SidebarButton::Item(item),
                MouseButton::Right,
                &menu,
                available,
                InterfaceMode::Normal
            ),
            SidebarEffect::Nothing
        );
        // A locked item is not queued; a ready building starts placement mode instead.
        assert_eq!(
            button_effect(
                SidebarButton::Item(item),
                MouseButton::Left,
                &menu,
                ItemStatus::default(),
                InterfaceMode::Normal
            ),
            SidebarEffect::Nothing
        );
        let ready = ItemStatus {
            ready: true,
            queued: 1,
            progress: Some((200, 200)),
            ..available
        };
        assert_eq!(
            button_effect(
                SidebarButton::Item(item),
                MouseButton::Left,
                &menu,
                ready,
                InterfaceMode::Normal
            ),
            SidebarEffect::Mode(InterfaceMode::Place(power))
        );
        // A ready building can still be cancelled with a right click.
        assert_eq!(
            button_effect(
                SidebarButton::Item(item),
                MouseButton::Right,
                &menu,
                ready,
                InterfaceMode::Normal
            ),
            SidebarEffect::Commands(vec![Command::CancelProduction { item }])
        );
        // Units are queued the same way.
        let tank = ProductionItem::Unit(definitions.unit_kind("light_tank").expect("tank"));
        assert_eq!(
            button_effect(
                SidebarButton::Item(tank),
                MouseButton::Left,
                &menu,
                available,
                InterfaceMode::Normal
            ),
            SidebarEffect::Commands(vec![Command::QueueProduction { item: tank }])
        );
        // An item not on the menu does nothing.
        let unknown = ProductionItem::Unit(UnitKind(999));
        assert_eq!(
            button_effect(
                SidebarButton::Item(unknown),
                MouseButton::Left,
                &menu,
                available,
                InterfaceMode::Normal
            ),
            SidebarEffect::Nothing
        );
    }

    #[test]
    fn mode_tab_pause_and_volume_buttons() {
        let menu = [];
        let none = ItemStatus::default();
        let effect = |button, mode| button_effect(button, MouseButton::Left, &menu, none, mode);
        assert_eq!(
            effect(SidebarButton::Sell, InterfaceMode::Normal),
            SidebarEffect::Mode(InterfaceMode::Sell)
        );
        assert_eq!(
            effect(SidebarButton::Sell, InterfaceMode::Sell),
            SidebarEffect::Mode(InterfaceMode::Normal)
        );
        assert_eq!(
            effect(SidebarButton::Rally, InterfaceMode::Sell),
            SidebarEffect::Mode(InterfaceMode::Rally)
        );
        assert_eq!(
            effect(SidebarButton::Rally, InterfaceMode::Rally),
            SidebarEffect::Mode(InterfaceMode::Normal)
        );
        assert_eq!(
            effect(SidebarButton::Tab(Tab::Units), InterfaceMode::Normal),
            SidebarEffect::Tab(Tab::Units)
        );
        assert_eq!(
            effect(SidebarButton::Pause, InterfaceMode::Normal),
            SidebarEffect::TogglePause
        );
        assert_eq!(
            effect(
                SidebarButton::Volume(Channel::Music, true),
                InterfaceMode::Normal
            ),
            SidebarEffect::Volume(Channel::Music, true)
        );
        assert_eq!(
            effect(SidebarButton::Mute, InterfaceMode::Normal),
            SidebarEffect::ToggleMute
        );
        // A right click does nothing on these buttons.
        assert_eq!(
            button_effect(
                SidebarButton::Pause,
                MouseButton::Right,
                &menu,
                none,
                InterfaceMode::Normal
            ),
            SidebarEffect::Nothing
        );
        assert_eq!(Tab::Buildings.next(), Tab::Units);
        assert_eq!(Tab::Units.next(), Tab::Buildings);
    }

    #[test]
    fn item_status_follows_the_players_buildings_and_queue() {
        // Through the simulation: the demo match, whose human player owns a construction yard.
        let mut host = crate::sim_host::tests::demo_host();
        let definitions = host.definitions().expect("definitions").clone();
        let power = ProductionItem::Building(definitions.building_kind("power_plant").expect("pp"));
        let refinery =
            ProductionItem::Building(definitions.building_kind("refinery").expect("refinery"));
        let tank = ProductionItem::Unit(definitions.unit_kind("light_tank").expect("tank"));
        let status = |host: &SimHost, item| {
            statuses(host)
                .into_iter()
                .find(|(entry, _)| *entry == item)
                .map(|(_, status)| status)
                .expect("on the menu")
        };
        assert_eq!(
            status(&host, power),
            ItemStatus {
                available: true,
                ..ItemStatus::default()
            }
        );
        // The refinery needs a power plant, the tank a war factory.
        assert!(!status(&host, refinery).available);
        assert!(!status(&host, tank).available);
        assert_eq!(badge_text(status(&host, refinery)), "locked");
        // Two power plants queued through the sink's path: the head progresses.
        crate::sim_host::tests::run(
            &mut host,
            vec![
                Command::QueueProduction { item: power },
                Command::QueueProduction { item: power },
            ],
        );
        for _ in 0..99 {
            crate::sim_host::tests::run(&mut host, Vec::new());
        }
        let half = status(&host, power);
        assert_eq!(half.queued, 2);
        assert_eq!(half.progress, Some((100, 200)));
        assert!(!half.ready);
        assert_eq!(badge_text(half), "50% +1");
        assert_eq!(progress_percent(half), 50.0);
        for _ in 0..100 {
            crate::sim_host::tests::run(&mut host, Vec::new());
        }
        let ready = status(&host, power);
        assert!(ready.ready);
        assert_eq!(badge_text(ready), "READY");
        assert_eq!(progress_percent(ready), 100.0);
        // The texts of the player's credits and power.
        let players = host.players().expect("players");
        let state = players.get(HUMAN);
        assert_eq!(credits_text(state), "Credits  4700");
        assert_eq!(power_text(state), "Power  0 / 20");
        assert_eq!(credits_text(None), "Credits  0");
        let _ = CellPos::new(0, 0);
    }

    #[test]
    fn texts_and_colours_of_the_sidebar() {
        let definitions = production_definitions();
        let power = ProductionItem::Building(definitions.building_kind("power_plant").expect("pp"));
        assert_eq!(item_label(power, &definitions), "power plant  $300");
        let tank = ProductionItem::Unit(definitions.unit_kind("light_tank").expect("tank"));
        assert_eq!(item_label(tank, &definitions), "light tank  $700");
        assert_eq!(badge_text(ItemStatus::default()), "locked");
        assert_eq!(
            badge_text(ItemStatus {
                available: true,
                ..ItemStatus::default()
            }),
            ""
        );
        // A build time of 0 cannot come from the data (at least 1, D23); it shows 0 %.
        assert_eq!(
            progress_percent(ItemStatus {
                progress: Some((5, 0)),
                ..ItemStatus::default()
            }),
            0.0
        );
        assert!(mode_text(InterfaceMode::Normal, true, None).starts_with("Paused"));
        assert!(
            mode_text(
                InterfaceMode::Place(BuildingKind(1)),
                false,
                Some(&definitions)
            )
            .contains("power plant")
        );
        let volume = VolumeSettings::default();
        assert_eq!(volume_text(&volume, Channel::Music), "Music 5/10");
        // Distinct states have distinct colours, and hovering lightens.
        let ready = ItemStatus {
            ready: true,
            ..ItemStatus::default()
        };
        let item = SidebarButton::Item(power);
        let colour = |status, hovered| {
            button_colour(
                item,
                status,
                Tab::Buildings,
                InterfaceMode::Normal,
                false,
                hovered,
            )
        };
        assert_ne!(colour(ready, false), colour(ItemStatus::default(), false));
        assert_ne!(colour(ready, false), colour(ready, true));
        assert_ne!(
            button_colour(
                SidebarButton::Pause,
                ItemStatus::default(),
                Tab::Units,
                InterfaceMode::Normal,
                true,
                false
            ),
            button_colour(
                SidebarButton::Pause,
                ItemStatus::default(),
                Tab::Units,
                InterfaceMode::Normal,
                false,
                false
            )
        );
    }

    #[test]
    fn the_game_systems_share_no_state_without_an_order() {
        // As `selection::tests::no_two_game_systems_share_state_in_post_update_without_an_order`
        // and the fog's test, with every plugin of the game (M5 adds the simulation host, the
        // sidebar, the minimap, health bars, and audio; M6 the menu and end screen), for every
        // schedule they add systems to.
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            crate::sim_host::SimHostPlugin,
            crate::camera::CameraPlugin,
            crate::map::MapPlugin,
            crate::units::UnitsPlugin,
            crate::buildings::BuildingsPlugin,
            crate::selection::SelectionPlugin,
            crate::construction::ConstructionPlugin,
            crate::fog::FogPlugin,
            SidebarPlugin,
            crate::minimap::MinimapPlugin,
            crate::health::HealthBarsPlugin,
            crate::audio::GameAudioPlugin,
            crate::skirmish::SkirmishPlugin,
        ));
        let check = |world: &mut World, schedule: &mut Schedule| {
            schedule.initialize(world).expect("schedule builds");
            assert!(
                schedule.graph().conflicting_systems().is_empty(),
                "{:?}: {} conflicting pairs",
                schedule.label(),
                schedule.graph().conflicting_systems().len()
            );
        };
        app.world_mut().schedule_scope(Startup, check);
        app.world_mut().schedule_scope(FixedUpdate, check);
        app.world_mut().schedule_scope(Update, check);
        app.world_mut().schedule_scope(PostUpdate, check);
    }
}
