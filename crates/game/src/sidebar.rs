//! The sidebar (PLAN.md section 7, M5; docs/DECISIONS.md D35): the minimap, the human player's
//! credits and power, build tabs with a button per production item showing its cost, queue, and
//! progress, buttons for selling, rally points, and pause, and the volume settings.
//!
//! | Input | Effect |
//! |-------|--------|
//! | Left click on an item | `QueueProduction` of the item (if its prerequisites and producer are owned); on a building that is ready, placement mode (`crate::construction`) |
//! | Right click on an item | `CancelProduction` of the item (if one is queued) |
//! | Tabs, or `Tab` | show the buildings or the units |
//! | Sell, Repair, Rally | sell mode, repair mode, rally mode (`crate::construction`) |
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
pub const SIDEBAR_WIDTH: f32 = 264.0;

pub struct SidebarPlugin;

impl Plugin for SidebarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tab>()
            .init_resource::<BuildPage>()
            .add_systems(Startup, spawn_sidebar.after(crate::minimap::create_minimap))
            // Cameo cropping reads the sprite images that the minimap's drawing writes.
            .add_systems(
                Update,
                (
                    populate_items.before(crate::minimap::draw_minimap),
                    update_sidebar,
                )
                    .chain(),
            )
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
    Armory,
    Infantry,
    Vehicles,
}
#[derive(Resource, Default)]
pub struct BuildPage(pub usize);
const PAGE_SIZE: usize = 6;
impl Tab {
    pub fn of(item: ProductionItem, defs: &Definitions) -> Self {
        match defs.category(item) {
            sim::Category::Buildings => Self::Buildings,
            sim::Category::Armory => Self::Armory,
            sim::Category::Infantry => Self::Infantry,
            sim::Category::Vehicles => Self::Vehicles,
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::Buildings => Self::Armory,
            Self::Armory => Self::Infantry,
            Self::Infantry => Self::Vehicles,
            Self::Vehicles => Self::Buildings,
        }
    }
}

/// A button of the sidebar.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarButton {
    Item(ProductionItem),
    Page(bool),
    Tab(Tab),
    Sell,
    Repair,
    Rally,
    Pause,
    /// Lowers (`false`) or raises (`true`) a volume channel.
    Volume(Channel, bool),
    Mute,
    /// Slower (`false`) or faster (`true`) game speed (`crate::game_speed`).
    Speed(bool),
}

/// A text of the sidebar that changes during the match.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarText {
    Credits,
    Details,
    PageLabel,
    Power,
    Mode,
    Badge(ProductionItem),
    Volume(Channel),
    PauseLabel,
    MuteLabel,
    Speed,
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
    /// The roster of the player's faction offers the item (M7), and the player owns every
    /// prerequisite and a producer, so `QueueProduction` is accepted (unless the queue is full).
    pub available: bool,
    /// Entries of the item in its queue.
    pub queued: usize,
    /// Progress and build time of the item if it is at the head of its queue.
    pub progress: Option<(u32, u32)>,
    /// A building that is complete and waits for placement.
    pub ready: bool,
}

/// The status of `item` for a player with `state` who owns buildings of the types `owned`. An item
/// the roster of the player's faction does not offer is never available (M7).
pub fn item_status(
    item: ProductionItem,
    definitions: &Definitions,
    owned: &BTreeSet<BuildingKind>,
    state: &PlayerState,
) -> ItemStatus {
    let Some(terms) = definitions.production(item) else {
        return ItemStatus::default();
    };
    // Exactly the checks of `QueueProduction` before the length of the queue (RA1: the
    // buildability rule, `sim::check_buildable`). Superweapons count as switched on, which they
    // are in every match the game starts until the menu offers the option.
    let owned_kinds: Vec<BuildingKind> = owned.iter().copied().collect();
    let available = sim::check_buildable(definitions, state, true, &owned_kinds, item).is_ok();
    let queue = state.queue(definitions.queue_key(terms.producer));
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
    Page(bool),
    Commands(Vec<Command>),
    Mode(InterfaceMode),
    Tab(Tab),
    TogglePause,
    Volume(Channel, bool),
    ToggleMute,
    Speed(bool),
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
        SidebarButton::Page(next) => SidebarEffect::Page(next),
        SidebarButton::Tab(tab) => SidebarEffect::Tab(tab),
        SidebarButton::Sell => SidebarEffect::Mode(if mode == InterfaceMode::Sell {
            InterfaceMode::Normal
        } else {
            InterfaceMode::Sell
        }),
        SidebarButton::Repair => SidebarEffect::Mode(if mode == InterfaceMode::Repair {
            InterfaceMode::Normal
        } else {
            InterfaceMode::Repair
        }),
        SidebarButton::Rally => SidebarEffect::Mode(if mode == InterfaceMode::Rally {
            InterfaceMode::Normal
        } else {
            InterfaceMode::Rally
        }),
        SidebarButton::Pause => SidebarEffect::TogglePause,
        SidebarButton::Volume(channel, up) => SidebarEffect::Volume(channel, up),
        SidebarButton::Mute => SidebarEffect::ToggleMute,
        SidebarButton::Speed(up) => SidebarEffect::Speed(up),
    }
}

/// The name of an item and its cost, e.g. `power plant  $300`.
pub fn availability_text(
    item: ProductionItem,
    defs: &Definitions,
    owned: &BTreeSet<BuildingKind>,
    state: Option<&PlayerState>,
) -> String {
    let Some(p) = defs.production(item) else {
        return "Unavailable".into();
    };
    // The producer is always needed (units leave it): a building that produces the item, or
    // with factory groups (RA1) a factory of its group and of a side that builds it. With
    // "unlock all" nothing else is needed; otherwise every prerequisite of the economy file and
    // a member of every prerequisite group of the arsenal (named by its first member).
    let waived = state.is_some_and(PlayerState::unlock_all);
    let unmet = |k: &BuildingKind| match defs.rules() {
        Some(_) => !owned.contains(k),
        None => !defs.prerequisite_met(*k, owned.iter().copied()),
    };
    let groups: Vec<BuildingKind> = defs
        .metadata(item)
        .zip(defs.rules())
        .map(|(entry, rules)| {
            entry
                .prerequisite_groups
                .iter()
                .filter_map(|&group| rules.group(group))
                .filter(|group| !group.buildings.iter().any(|k| owned.contains(k)))
                .filter_map(|group| group.buildings.first().copied())
                .collect()
        })
        .unwrap_or_default();
    let missing: Vec<_> = std::iter::once(&p.producer)
        .filter(|_| !owned.iter().any(|&k| sim::produces(defs, k, item)))
        .chain(p.prerequisites.iter().filter(|k| !waived && unmet(k)))
        .chain(groups.iter().filter(|_| !waived))
        .map(|k| {
            defs.metadata(ProductionItem::Building(*k)).map_or_else(
                || {
                    defs.building(*k)
                        .map_or("?".into(), |d| d.id.replace('_', " "))
                },
                |m| m.label.clone(),
            )
        })
        .collect();
    if !missing.is_empty() {
        return format!("Needs {}", missing.join(", "));
    }
    if let Some(s) = state {
        if s.queue(defs.queue_key(p.producer)).len() >= sim::MAX_QUEUE_LENGTH {
            return "Queue full".into();
        }
        if s.credits() == 0 {
            return "Waiting for credits".into();
        }
        if s.low_power() {
            return "Low power: half speed".into();
        }
    }
    format!(
        "{} seconds • ready to queue",
        p.build_time
            .div_ceil(state.map_or(1, PlayerState::production_speed))
            .div_ceil(20)
    )
}

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
        InterfaceMode::Interact => {
            "Interact: click a structure or friendly transport. Esc cancels.".into()
        }
        InterfaceMode::Strategic(_) => {
            "Strategic ability: click a visible target. Esc cancels.".into()
        }
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
        InterfaceMode::Repair => {
            "Repair (K): click a building to start or stop\nRight click or Esc: done".into()
        }
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
    // RA2-style gunmetal: lit amber for selection/readiness, dark recessed cameos when locked.
    let base: (f32, f32, f32) = match button {
        SidebarButton::Item(_) if status.ready => (0.42, 0.33, 0.10),
        SidebarButton::Item(_) if status.progress.is_some() || status.queued > 0 => {
            (0.13, 0.22, 0.20)
        }
        SidebarButton::Item(_) if !status.available => (0.055, 0.06, 0.065),
        SidebarButton::Item(_) => (0.12, 0.13, 0.135),
        SidebarButton::Tab(shown) if shown == tab => (0.50, 0.34, 0.09),
        SidebarButton::Sell if mode == InterfaceMode::Sell => (0.55, 0.16, 0.10),
        SidebarButton::Repair if mode == InterfaceMode::Repair => (0.14, 0.30, 0.52),
        SidebarButton::Rally if mode == InterfaceMode::Rally => (0.18, 0.42, 0.18),
        SidebarButton::Pause if paused => (0.58, 0.42, 0.08),
        _ => (0.21, 0.225, 0.235),
    };
    let lift = if hovered { 0.07 } else { 0.0 };
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

/// Whether the item's button is shown: the item belongs to the shown build tab and the roster of
/// the human player's faction offers it (M7; the other faction's items are not shown). Without a
/// match every item of the tab is shown.
pub fn item_shown(
    item: ProductionItem,
    tab: Tab,
    state: Option<&PlayerState>,
    defs: &Definitions,
) -> bool {
    Tab::of(item, defs) == tab && state.is_none_or(|state| state.offers(item))
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
            flex_shrink: 0.0,
            border: UiRect::all(Val::Px(2.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        bevel(true),
        BackgroundColor(Color::srgb(0.21, 0.225, 0.235)),
    )
}

/// Machined-metal edge: light from the upper left, like the battlefield's sun.
fn bevel(raised: bool) -> BorderColor {
    let (light, dark) = (
        Color::srgb(0.55, 0.57, 0.58),
        Color::srgb(0.03, 0.035, 0.04),
    );
    let (a, b) = if raised { (light, dark) } else { (dark, light) };
    BorderColor {
        top: a,
        left: a,
        bottom: b,
        right: b,
    }
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
                row_gap: Val::Px(2.0),
                border: UiRect::left(Val::Px(4.0)),
                ..default()
            },
            bevel(true),
            BackgroundColor(Color::srgb(0.135, 0.145, 0.15)),
        ))
        .with_children(|sidebar| {
            sidebar
                .spawn(text("BATTLE CONTROL", 15.0))
                .insert(TextColor(Color::srgb(0.95, 0.74, 0.30)));
            sidebar.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(4.0),
                    flex_shrink: 0.0,
                    border: UiRect::vertical(Val::Px(1.0)),
                    ..default()
                },
                bevel(false),
                BackgroundColor(Color::srgb(0.06, 0.065, 0.07)),
            ));
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
            sidebar.spawn((SidebarText::Credits, text("Credits", 17.0)));
            sidebar.spawn((SidebarText::Power, text("Power", 12.0)));
            sidebar
                .spawn(Node {
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    for (tab, label) in [
                        (Tab::Buildings, "Buildings"),
                        (Tab::Armory, "Armory"),
                        (Tab::Infantry, "Infantry"),
                        (Tab::Vehicles, "Vehicles"),
                    ] {
                        row.spawn(button(SidebarButton::Tab(tab), Val::Percent(24.0), 25.0))
                            .with_child(text(label, 11.0));
                    }
                });
            sidebar.spawn((
                ItemList,
                Node {
                    flex_wrap: FlexWrap::Wrap,
                    height: Val::Px(308.0),
                    flex_shrink: 0.0,
                    align_content: AlignContent::Start,
                    column_gap: Val::Px(4.0),
                    row_gap: Val::Px(4.0),
                    ..default()
                },
            ));
            sidebar
                .spawn(Node {
                    column_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn(button(SidebarButton::Page(false), Val::Px(44.0), 20.0))
                        .with_child(text("<", 12.0));
                    row.spawn((
                        SidebarText::PageLabel,
                        text("", 11.0),
                        Node {
                            width: Val::Px(140.0),
                            ..default()
                        },
                    ));
                    row.spawn(button(SidebarButton::Page(true), Val::Px(44.0), 20.0))
                        .with_child(text(">", 12.0));
                });
            sidebar.spawn((
                SidebarText::Details,
                text(
                    "Hover an item for prerequisites and role. D deploy / unload. E interact.",
                    10.0,
                ),
                Node {
                    height: Val::Px(36.0),
                    ..default()
                },
            ));
            sidebar
                .spawn(Node {
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn(button(SidebarButton::Sell, Val::Percent(24.0), 22.0))
                        .with_child(text("Sell", 12.0));
                    row.spawn(button(SidebarButton::Repair, Val::Percent(24.0), 22.0))
                        .with_child(text("Repair", 12.0));
                    row.spawn(button(SidebarButton::Rally, Val::Percent(24.0), 22.0))
                        .with_child(text("Rally", 12.0));
                    row.spawn(button(SidebarButton::Pause, Val::Percent(24.0), 22.0))
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
                            16.0,
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
                            16.0,
                        ))
                        .with_child(text("+", 13.0));
                    });
            }
            sidebar
                .spawn(Node {
                    column_gap: Val::Px(4.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn(button(SidebarButton::Mute, Val::Px(64.0), 18.0))
                        .with_child((SidebarText::MuteLabel, text("Mute", 13.0)));
                    row.spawn(button(SidebarButton::Speed(false), Val::Px(24.0), 18.0))
                        .with_child(text("-", 13.0));
                    row.spawn((
                        SidebarText::Speed,
                        text("Speed 1x", 13.0),
                        Node {
                            width: Val::Px(96.0),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                    ));
                    row.spawn(button(SidebarButton::Speed(true), Val::Px(24.0), 18.0))
                        .with_child(text("+", 13.0));
                });
            sidebar.spawn((SidebarText::Mode, text("", 10.0)));
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

/// Largest cameo on a production card, in logical pixels.
const CAMEO_MAX_WIDTH: f32 = 108.0;
const CAMEO_HEIGHT: f32 = 50.0;

/// Adds a button per production item once the match is set up.
fn populate_items(
    mut commands: Commands,
    host: Res<SimHost>,
    buildings: Option<Res<crate::buildings::BuildingSprites>>,
    units: Option<Res<crate::units::UnitSprites>>,
    images: Res<Assets<Image>>,
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
                // The cameo shows the miniature cropped to its solid pixels, as large as the card
                // allows, so similar-sized items are told apart by shape at a glance.
                let image = match item {
                    ProductionItem::Building(kind) => {
                        buildings.as_ref().and_then(|s| s.0.get(&kind)).cloned()
                    }
                    ProductionItem::Unit(kind) => {
                        units.as_ref().and_then(|s| s.0.get(&kind)).cloned()
                    }
                };
                let icon = image.map(|handle| {
                    let full = match item {
                        ProductionItem::Unit(_) => {
                            crate::hd_art::facing_rect(sim::Direction::ALL[0])
                        }
                        ProductionItem::Building(_) => {
                            images
                                .get(&handle)
                                .map_or(Rect::new(0.0, 0.0, 1.0, 1.0), |image| {
                                    Rect::new(0.0, 0.0, image.width() as f32, image.height() as f32)
                                })
                        }
                    };
                    let crop = images
                        .get(&handle)
                        .map_or(full, |image| crate::hd_art::content_rect(image, full));
                    let size = crate::hd_art::fit(crop, Vec2::new(CAMEO_MAX_WIDTH, CAMEO_HEIGHT));
                    (ImageNode::new(handle).with_rect(crop), size)
                });
                list.spawn((
                    SidebarButton::Item(item),
                    Button,
                    Node {
                        width: Val::Px(118.0),
                        height: Val::Px(100.0),
                        flex_shrink: 0.0,
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        border: UiRect::all(Val::Px(2.0)),
                        padding: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    bevel(true),
                    BackgroundColor(Color::srgb(0.12, 0.13, 0.135)),
                ))
                .with_children(|entry| {
                    if let Some((icon, size)) = icon {
                        entry.spawn((
                            icon,
                            Node {
                                width: Val::Px(size.x),
                                height: Val::Px(size.y),
                                ..default()
                            },
                        ));
                    }
                    let label = definitions.metadata(item).map_or_else(
                        || item_label(item, definitions),
                        |e| {
                            format!(
                                "{}  ${}",
                                e.label,
                                definitions.production(item).map_or(0, |p| host
                                    .players()
                                    .and_then(|p| p.get(HUMAN))
                                    .map_or(p.cost, |state| state.production_cost(p.cost)))
                            )
                        },
                    );
                    let (name, cost) = label.split_once("  ").unwrap_or((&label, ""));
                    entry.spawn(text(name, 11.0));
                    entry
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            justify_content: JustifyContent::SpaceBetween,
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|line| {
                            line.spawn(text(cost, 11.0))
                                .insert(TextColor(Color::srgb(0.89, 0.78, 0.51)));
                            line.spawn((SidebarText::Badge(item), text("", 10.0)));
                        });
                    entry
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(2.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.05, 0.07, 0.07)),
                        ))
                        .with_child((
                            ItemProgress(item),
                            Node {
                                width: Val::Percent(0.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.95, 0.72, 0.22)),
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
    mut page: ResMut<BuildPage>,
    mut mode: ResMut<InterfaceMode>,
    mut sink: ResMut<CommandSink>,
    mut volume: ResMut<VolumeSettings>,
    mut speed: ResMut<crate::game_speed::GameSpeed>,
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
            Hotkey::SpeedDown => SidebarEffect::Speed(false),
            Hotkey::SpeedUp => SidebarEffect::Speed(true),
            _ => SidebarEffect::Nothing,
        });
    }
    for effect in effects {
        match effect {
            SidebarEffect::Nothing => {}
            SidebarEffect::Page(next) => {
                if next {
                    page.0 = page.0.saturating_add(1);
                } else {
                    page.0 = page.0.saturating_sub(1);
                }
            }
            SidebarEffect::Commands(commands) => {
                sink.issue(commands);
            }
            SidebarEffect::Mode(next) => *mode = next,
            SidebarEffect::Tab(next) => {
                *tab = next;
                page.0 = 0;
            }
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
            SidebarEffect::Speed(up) => {
                if speed.step(up) {
                    info!("game speed {}", speed.label());
                }
            }
        }
    }
}

/// Brings the sidebar's texts, colours, progress bars, and the pause banner up to date.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn update_sidebar(
    host: Res<SimHost>,
    sink: Res<CommandSink>,
    tab: Res<Tab>,
    mut page: ResMut<BuildPage>,
    mode: Res<InterfaceMode>,
    volume: Res<VolumeSettings>,
    speed: Res<crate::game_speed::GameSpeed>,
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
    let defs = host.definitions();
    let owned = owned_kinds(&host);
    let listed: Vec<_> = defs
        .map(|d| {
            production_menu(d)
                .into_iter()
                .filter(|&i| item_shown(i, *tab, state, d))
                .collect()
        })
        .unwrap_or_default();
    let pages = listed.len().div_ceil(PAGE_SIZE).max(1);
    page.0 = page.0.min(pages - 1);
    let shown: Vec<_> = listed
        .into_iter()
        .skip(page.0 * PAGE_SIZE)
        .take(PAGE_SIZE)
        .collect();
    let hovered = buttons.iter().find_map(|(b, i, _, n)| {
        if *i != Interaction::None && n.display != Display::None {
            if let SidebarButton::Item(item) = b {
                Some(*item)
            } else {
                None
            }
        } else {
            None
        }
    });
    for (mut text, mut colour, kind) in &mut texts {
        let value = match kind {
            SidebarText::PageLabel => format!("{} / {}  |  Tab cycles",page.0+1,pages),
            SidebarText::Details => hovered.and_then(|i|defs.map(|d| {
                let description=d.metadata(i).map_or("",|e|e.description.as_str());
                format!("{}\n{}",availability_text(i,d,&owned,state),description)
            })).unwrap_or_else(||"Hover: requirements / role\nSpace + drag: camera | D deploy\nRight click card: cancel / refund".into()),
            SidebarText::Credits => credits_text(state),
            SidebarText::Power => power_text(state),
            SidebarText::Mode => mode_text(*mode, paused, host.definitions()),
            // Queued progress stays visible even if a prerequisite was lost meanwhile.
            SidebarText::Badge(item) => {
                let status = status_of(*item);
                if status.available || status.ready || status.queued > 0 {
                    badge_text(status)
                } else {
                    "LOCKED".into()
                }
            }
            SidebarText::Volume(channel) => volume_text(&volume, *channel),
            SidebarText::PauseLabel => if paused { "Resume" } else { "Pause" }.into(),
            SidebarText::MuteLabel => if volume.muted { "Unmute" } else { "Mute" }.into(),
            SidebarText::Speed => format!("Speed {}", speed.label()),
        };
        if text.0 != value {
            text.0 = value;
        }
        let wanted = match kind {
            SidebarText::Power if state.is_some_and(PlayerState::low_power) => {
                Color::srgb(1.0, 0.36, 0.26)
            }
            SidebarText::Power => Color::srgb(0.55, 0.92, 0.42),
            SidebarText::Credits => Color::srgb(1.0, 0.80, 0.32),
            SidebarText::Speed if speed.factor() != 1.0 => Color::srgb(1.0, 0.80, 0.32),
            _ => Color::srgb(0.86, 0.88, 0.86),
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
            let display = if shown.contains(item) {
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
        assert!(!over_sidebar(
            Vec2::new(1280.0 - SIDEBAR_WIDTH - 0.1, 10.0),
            1280.0
        ));
        assert!(over_sidebar(
            Vec2::new(1280.0 - SIDEBAR_WIDTH, 10.0),
            1280.0
        ));
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
        let tank = ProductionItem::Unit(definitions.unit_kind("grizzly").expect("tank"));
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
            effect(SidebarButton::Tab(Tab::Infantry), InterfaceMode::Normal),
            SidebarEffect::Tab(Tab::Infantry)
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
        assert_eq!(Tab::Buildings.next(), Tab::Armory);
        assert_eq!(Tab::Armory.next(), Tab::Infantry);
        assert_eq!(Tab::Infantry.next(), Tab::Vehicles);
        assert_eq!(Tab::Vehicles.next(), Tab::Buildings);
    }

    #[test]
    fn item_status_follows_the_players_buildings_and_queue() {
        // Through the simulation: the demo match, whose human player owns a construction yard
        // (RA1: `demo_host` starts the players with the yards their base units deploy into).
        let mut host = crate::sim_host::tests::demo_host();
        let definitions = host.definitions().expect("definitions").clone();
        let power = ProductionItem::Building(definitions.building_kind("power_plant").expect("pp"));
        let refinery =
            ProductionItem::Building(definitions.building_kind("refinery").expect("refinery"));
        let tank = ProductionItem::Unit(definitions.unit_kind("grizzly").expect("tank"));
        // The cost and build time of the power plant are data (Red Alert 2's since RA1), as are
        // the starting credits and the power the yard supplies.
        let terms = definitions.production(power).expect("terms").clone();
        let (cost, total) = (terms.cost, terms.build_time);
        assert!(total >= 2 && total % 2 == 0, "{total}");
        let credits = host.players().and_then(|players| players.get(HUMAN));
        let credits = credits.map_or(0, PlayerState::credits);
        let supplied: i32 = owned_kinds(&host)
            .iter()
            .filter_map(|&kind| definitions.building(kind))
            .map(|def| def.power_produced)
            .sum();
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
        for _ in 1..total / 2 {
            crate::sim_host::tests::run(&mut host, Vec::new());
        }
        let half = status(&host, power);
        assert_eq!(half.queued, 2);
        assert_eq!(half.progress, Some((total / 2, total)));
        assert!(!half.ready);
        assert_eq!(badge_text(half), "50% +1");
        assert_eq!(progress_percent(half), 50.0);
        for _ in 0..total / 2 {
            crate::sim_host::tests::run(&mut host, Vec::new());
        }
        let ready = status(&host, power);
        assert!(ready.ready);
        assert_eq!(badge_text(ready), "READY");
        assert_eq!(progress_percent(ready), 100.0);
        // The texts of the player's credits and power.
        let players = host.players().expect("players");
        let state = players.get(HUMAN);
        assert!(cost > 0 && cost < credits, "{cost} of {credits}");
        assert_eq!(credits_text(state), format!("Credits  {}", credits - cost));
        assert_eq!(power_text(state), format!("Power  0 / {supplied}"));
        assert_eq!(credits_text(None), "Credits  0");
        let _ = CellPos::new(0, 0);
    }

    #[test]
    fn items_outside_the_roster_of_the_players_faction_are_neither_available_nor_shown() {
        // M7: the factions offer different items; with every building owned, an item is
        // available exactly when the player's roster offers it, and its button is shown exactly
        // then (on its tab). Since RA1 the shipped factions are countries, whose players have no
        // roster (the buildability rule decides), so this runs on the legacy ruleset fixture: the
        // Vanguard and the Meridian with their rosters.
        let catalogue = crate::skirmish::tests::legacy_catalogue();
        let definitions = &catalogue.definitions;
        let owned: BTreeSet<BuildingKind> = (0..definitions.buildings().len())
            .map(|index| BuildingKind(index as u16))
            .collect();
        let menu = production_menu(definitions);
        assert!(catalogue.skirmish.factions.len() >= 2);
        let mut offered_by_one_only = 0;
        for faction in &catalogue.skirmish.factions {
            let mut players = sim::Players::default();
            players
                .add_with_roster(HUMAN, 0, faction.roster.clone())
                .expect("player");
            let state = players.get(HUMAN).expect("player");
            for &item in &menu {
                let offered = faction.offers(item);
                let status = item_status(item, definitions, &owned, state);
                assert_eq!(status.available, offered, "{}: {item:?}", faction.id);
                for tab in [Tab::Buildings, Tab::Armory, Tab::Infantry, Tab::Vehicles] {
                    assert_eq!(
                        item_shown(item, tab, Some(state), definitions),
                        offered && Tab::of(item, definitions) == tab,
                        "{}: {item:?}",
                        faction.id
                    );
                }
                if !offered {
                    offered_by_one_only += 1;
                    // A left click on a button that is not available does nothing.
                    assert_eq!(
                        button_effect(
                            SidebarButton::Item(item),
                            MouseButton::Left,
                            &menu,
                            status,
                            InterfaceMode::Normal,
                        ),
                        SidebarEffect::Nothing
                    );
                }
            }
        }
        assert!(offered_by_one_only > 0, "the factions differ");
        // Without a match every item of the tab is shown.
        assert!(menu.iter().all(|&item| item_shown(
            item,
            Tab::of(item, definitions),
            None,
            definitions
        )));
    }

    #[test]
    fn texts_and_colours_of_the_sidebar() {
        let definitions = production_definitions();
        let power = ProductionItem::Building(definitions.building_kind("power_plant").expect("pp"));
        // The costs are data (Red Alert 2's since RA1).
        let cost = |item| definitions.production(item).expect("terms").cost;
        assert_eq!(
            item_label(power, &definitions),
            format!("power plant  ${}", cost(power))
        );
        let tank = ProductionItem::Unit(definitions.unit_kind("prism_tank").expect("tank"));
        assert_eq!(
            item_label(tank, &definitions),
            format!("prism tank  ${}", cost(tank))
        );
        assert!(cost(power) > 0 && cost(tank) > 0);
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
                Tab::Infantry,
                InterfaceMode::Normal,
                true,
                false
            ),
            button_colour(
                SidebarButton::Pause,
                ItemStatus::default(),
                Tab::Infantry,
                InterfaceMode::Normal,
                false,
                false
            )
        );
    }

    #[test]
    fn cards_follow_the_tech_tree_and_unlock_all_waives_only_prerequisites() {
        let catalogue = crate::skirmish::tests::production_catalogue();
        let defs = &catalogue.definitions;
        let tab_order = |tab: Tab| -> Vec<String> {
            production_menu(defs)
                .into_iter()
                .filter(|&item| Tab::of(item, defs) == tab)
                .map(|item| match item {
                    ProductionItem::Building(k) => defs.building(k).unwrap().id.clone(),
                    ProductionItem::Unit(k) => defs.unit(k).unwrap().id.clone(),
                })
                .collect()
        };
        let buildings = tab_order(Tab::Buildings);
        let at = |list: &[String], id: &str| list.iter().position(|e| e == id).expect(id);
        assert_eq!(buildings[0], "construction_yard");
        // What each structure unlocks comes after it, and the Radar Tower that the Soviet radar
        // units need is no longer on the last page.
        assert!(at(&buildings, "power_plant") < at(&buildings, "refinery"));
        assert!(at(&buildings, "refinery") < at(&buildings, "radar_tower"));
        assert!(at(&buildings, "radar_tower") < at(&buildings, "tech_center"));
        // (Since RA1 the tab lists the buildings of both sides, on four pages.)
        let last_page = (buildings.len() - 1) / PAGE_SIZE * PAGE_SIZE;
        assert!(at(&buildings, "radar_tower") < last_page, "{buildings:?}");
        // The sides' armed towers are listed with the other defences.
        let armory = tab_order(Tab::Armory);
        at(&armory, "pillbox");
        at(&armory, "sentry_gun");
        let infantry = tab_order(Tab::Infantry);
        assert!(at(&infantry, "conscript") < at(&infantry, "tesla_trooper"));
        assert!(at(&infantry, "tesla_trooper") < at(&infantry, "yuri"));

        // The status of an item: prerequisites waived with "unlock all", never the producer.
        // The human player of the demo match plays America (RA1) and owns its yard: the items
        // are the Allied counterparts of the Tesla Coil and the Apocalypse.
        let host = crate::sim_host::tests::demo_host();
        let owned = owned_kinds(&host);
        let mut state = host.players().unwrap().get(HUMAN).unwrap().clone();
        let coil = ProductionItem::Building(defs.building_kind("prism_tower").unwrap());
        let tank = ProductionItem::Unit(defs.unit_kind("prism_tank").unwrap());
        assert!(!item_status(coil, defs, &owned, &state).available);
        assert!(availability_text(coil, defs, &owned, Some(&state)).starts_with("Needs"));
        let unlocked = {
            let mut app = App::new();
            app.add_plugins(sim::SimPlugin { seed: 0 });
            app.world_mut()
                .insert_resource(catalogue.maps[0].map.grid.clone());
            app.world_mut()
                .insert_resource(catalogue.definitions.clone());
            let selection = crate::skirmish::MenuSelection::initial(&catalogue);
            sim::setup_match_with_options(
                app.world_mut(),
                &catalogue.skirmish,
                &catalogue.ai,
                0,
                &selection.players(),
                &[],
                &[HUMAN],
            )
            .unwrap();
            app.world()
                .resource::<sim::Players>()
                .get(HUMAN)
                .unwrap()
                .clone()
        };
        assert!(unlocked.unlock_all());
        state = unlocked;
        assert!(item_status(coil, defs, &owned, &state).available);
        assert!(
            !item_status(tank, defs, &owned, &state).available,
            "needs a war factory"
        );
        assert_eq!(
            availability_text(tank, defs, &owned, Some(&state)),
            "Needs War Factory"
        );
        // Progress of a queued item stays visible even while it is not available.
        let queued = ItemStatus {
            available: false,
            queued: 1,
            progress: Some((5, 10)),
            ready: false,
        };
        assert_eq!(badge_text(queued), "50%");
    }

    #[test]
    fn the_game_systems_share_no_state_without_an_order() {
        // As `selection::tests::no_two_game_systems_share_state_in_post_update_without_an_order`
        // and the fog's test, with every plugin of the game (M5 adds the simulation host, the
        // sidebar, the minimap, health bars, and audio; M6 the menu and end screen; M7 the
        // options), for every schedule they add systems to.
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
        ))
        .add_plugins(crate::options::OptionsPlugin { path: None })
        .add_plugins(crate::special_controls::SpecialControlsPlugin)
        .add_plugins(crate::hover_info::HoverInfoPlugin);
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
        app.world_mut().schedule_scope(Last, check);
    }
}
