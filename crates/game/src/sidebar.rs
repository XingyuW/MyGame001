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
//! | Pause, `Pause`, `F9`, or backslash | pause or resume (`crate::sim_host::CommandSink`) |
//! | Volume buttons, `-`, `=`, `F10`, apostrophe | master, music, and effects volume, mute (`crate::audio`) |
//!
//! Since RA1 a tab lists, as Red Alert 2 shows a cameo, only the items that some building the
//! player owns could build ([`item_listed`]): the player owns a factory of the item's kind and
//! of a side that builds it, the item is not closed to the player's country, and every
//! prerequisite is owned or can be built with what the player owns. An item whose prerequisites
//! are missing but within reach stays listed as locked, with the reason; an Allied player sees
//! no Soviet card until it has captured a Soviet building, and nobody sees a card before the
//! MCV has deployed.
//!
//! Since RA2 the Construction Yard builds in two lanes, the buildings and the defences (the
//! Armory tab), one item each, and every factory group has a queue of its own. Each card shows
//! the progress of its own item, and each tab carries a bar with the progress of its lane
//! ([`tab_progress`]), so that the lane of the tab that is not shown is seen too. A card whose
//! lane is taken says `busy`, one whose build limit is reached `LIMIT`.
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
    Cost(ProductionItem),
    Volume(Channel),
    PauseLabel,
    MuteLabel,
    Speed,
    /// The "RADAR OFFLINE" panel over the minimap; empty while the player has radar (RA2).
    Radar,
}

/// The filled part of a progress bar: that of an item's card, or that on a tab's button, which
/// shows the lane of the tab (RA2, [`tab_progress`]).
#[derive(Component, Debug)]
pub enum ItemProgress {
    Item(ProductionItem),
    Tab(Tab),
}

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
    /// The player owns or has queued as many as the item's build limit allows (RA2).
    pub limited: bool,
    /// The queue of the item holds as many items as it can (RA2: a lane of the Construction
    /// Yard holds one building), so `QueueProduction` would be rejected as `QueueFull`.
    pub full: bool,
}

/// The status of `item` for a player with `state` who owns buildings of the types `owned`, in a
/// match with superweapons switched on or off (`superweapons`, the menu's row). An item the
/// roster of the player's faction does not offer is never available (M7).
pub fn item_status(
    item: ProductionItem,
    definitions: &Definitions,
    owned: &BTreeSet<BuildingKind>,
    state: &PlayerState,
    superweapons: bool,
) -> ItemStatus {
    let Some(terms) = definitions.production(item) else {
        return ItemStatus::default();
    };
    // Exactly the checks of `QueueProduction` before the length of the queue (RA1: the
    // buildability rule, `sim::check_buildable`).
    let owned_kinds: Vec<BuildingKind> = owned.iter().copied().collect();
    let check = sim::check_buildable(definitions, state, superweapons, &owned_kinds, item);
    let available = check.is_ok();
    // RA4: an aircraft whose docks are all taken is listed like an item at its build limit.
    let limited = matches!(
        check,
        Err(sim::RejectReason::BuildLimitReached { .. } | sim::RejectReason::NoFreeDock(_))
    );
    // The queue of the item: of its factory group and, for a building, of its lane (RA2).
    let queue = definitions
        .queue_of(item)
        .map_or(&[][..], |key| state.lane(key));
    let full = queue.len() >= definitions.queue_capacity(item);
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
        limited,
        full,
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
            // A full queue takes nothing (RA2: the lane of a building in production).
            let command = if left && status.available && !status.full {
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

/// What keeps the item from being queued (`Needs ...`, a reached build limit, a full queue, no
/// credits), or how long it takes with `factories` working factories of its kind and the
/// player's power: under low power the rate of the rules (RA2), or half speed without a rules
/// file.
pub fn availability_text(
    item: ProductionItem,
    defs: &Definitions,
    owned: &BTreeSet<BuildingKind>,
    state: Option<&PlayerState>,
    factories: usize,
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
    if factories == 0 {
        return "Production paused: no working factory".into();
    }
    let rates = defs.rules().map(|rules| rules.production);
    if let Some(s) = state {
        let kinds: Vec<BuildingKind> = owned.iter().copied().collect();
        match sim::check_buildable(defs, s, true, &kinds, item) {
            Err(sim::RejectReason::BuildLimitReached { limit, .. }) => {
                return format!("Build limit reached: {limit} per player");
            }
            // RA4: every dock of the player's buildings holds an aircraft or waits for one.
            Err(sim::RejectReason::NoFreeDock(_)) => {
                return "No free dock: every dock is taken".into();
            }
            _ => {}
        }
        let queue = defs.queue_of(item).map_or(&[][..], |key| s.lane(key));
        if queue.len() >= defs.queue_capacity(item) {
            return match (item, rates) {
                (ProductionItem::Building(_), Some(_)) => {
                    "Busy: the yard builds one of these at a time".into()
                }
                _ => "Queue full".into(),
            };
        }
        if s.credits() == 0 {
            return "Waiting for credits".into();
        }
        if s.low_power() {
            return match &rates {
                Some(rates) => format!(
                    "Low power: production at {}%",
                    sim::power_rate_pm(rates, s.power_produced(), s.power_consumed()) / 10
                ),
                None => "Low power: half speed".into(),
            };
        }
    }
    format!(
        "{} seconds • ready to queue",
        build_ticks(p.build_time, defs, state, factories).div_ceil(20)
    )
}

/// Ticks an item of `build_time` takes for a player with `state` and `factories` working
/// factories of its kind: with a rules file the build time over the production rate
/// (`sim::production_rate_pm`, RA2), rounded up; without one the build time over the Super Me
/// speed.
pub fn build_ticks(
    build_time: u32,
    defs: &Definitions,
    state: Option<&PlayerState>,
    factories: usize,
) -> u32 {
    match defs.rules() {
        Some(rules) => {
            let (produced, consumed, super_me) = state.map_or((0, 0, false), |state| {
                (
                    state.power_produced(),
                    state.power_consumed(),
                    state.super_me(),
                )
            });
            let rate =
                sim::production_rate_pm(&rules.production, factories, produced, consumed, super_me)
                    .max(1);
            u32::try_from((u64::from(build_time) * 1000).div_ceil(rate)).unwrap_or(u32::MAX)
        }
        None => build_time.div_ceil(state.map_or(1, PlayerState::production_speed)),
    }
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

/// The badge of an item: `READY`, its progress in percent, how many are queued, `LIMIT` (its
/// build limit is reached), `locked`, or `busy` (its queue is full with something else).
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
    if parts.is_empty() {
        if status.limited {
            parts.push("LIMIT".into());
        } else if !status.available {
            parts.push("locked".into());
        } else if status.full {
            parts.push("busy".into());
        }
    }
    parts.join(" ")
}

/// Width of an item's progress bar in percent.
pub fn progress_percent(status: ItemStatus) -> f32 {
    status.progress.map_or(0.0, |(done, total)| {
        (done.min(total) as f32 / total.max(1) as f32) * 100.0
    })
}

/// The progress of the lane of `tab` in percent, for the bar on the tab's button (RA2: the
/// buildings and the defences of the Construction Yard progress side by side, and so do the
/// queues of the factories): the progress of the item of that tab that is in production, the
/// most advanced one if several queues belong to the tab (vehicles, ships and aircraft);
/// `None` while nothing of the tab is in production.
pub fn tab_progress(
    tab: Tab,
    statuses: &[(ProductionItem, ItemStatus)],
    defs: &Definitions,
) -> Option<f32> {
    statuses
        .iter()
        .filter(|(item, status)| status.progress.is_some() && Tab::of(*item, defs) == tab)
        .map(|(_, status)| progress_percent(*status))
        .reduce(f32::max)
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
        return "Paused: Pause / F9 / \u{005c} to resume".into();
    }
    match mode {
        InterfaceMode::Interact => {
            "Interact: click a structure, a friendly transport,\nor your depot with a damaged vehicle. Esc cancels."
                .into()
        }
        InterfaceMode::ChronoDestination(..) => "Chronosphere: choose a visible destination for the 3 x 3 source area. Esc cancels.".into(),
        InterfaceMode::Strategic(_) => {
            "Strategic ability: click a visible target. Chronosphere: choose the 3 x 3 source first. Esc cancels.".into()
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
        SidebarButton::Item(_) if !status.available || status.full => (0.055, 0.06, 0.065),
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
    let Some(mut buildings) = host
        .world()
        .try_query::<(&Owner, &Building, Option<&sim::Health>)>()
    else {
        return BTreeSet::new();
    };
    buildings
        .iter(host.world())
        .filter(|(owner, _, health)| {
            owner.0 == HUMAN && !health.is_some_and(|health| health.hit_points <= 0)
        })
        .map(|(_, building, _)| building.kind)
        .collect()
}

/// The number of the human player's buildings that are factories of the kind that produces `item`
/// (the number the production rate of RA2 counts, `sim::production_rate_pm`).
fn factories_of(host: &SimHost, item: ProductionItem) -> usize {
    let (Some(definitions), Some(mut buildings)) = (
        host.definitions(),
        host.world()
            .try_query::<(&sim::SimId, &Owner, &Building, Option<&sim::Health>)>(),
    ) else {
        return 1;
    };
    let Some(terms) = definitions.production(item) else {
        return 1;
    };
    buildings
        .iter(host.world())
        .filter(|(id, owner, building, health)| {
            owner.0 == HUMAN
                && !health.is_some_and(|health| health.hit_points <= 0)
                && !host
                    .world()
                    .get_resource::<sim::SpecialState>()
                    .is_some_and(|special| special.disabled.contains_key(*id))
                && definitions.same_factory(building.kind, terms.producer)
        })
        .count()
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

/// The building types a player with `state` who owns buildings of the types `owned` can come to
/// own by building alone: the owned ones, and every building type it could queue with them and
/// with those found so far (`sim::check_buildable`), until none is added. For a player with the
/// Construction Yard of its side that is the tech tree of its country
/// (`sim::country_tree`); a building that only a captured building opens is not in it.
pub fn reachable_buildings(
    definitions: &Definitions,
    owned: &BTreeSet<BuildingKind>,
    state: &PlayerState,
    superweapons: bool,
) -> Vec<BuildingKind> {
    let mut reached: Vec<BuildingKind> = owned.iter().copied().collect();
    let kinds = || {
        (0..definitions.buildings().len())
            .filter_map(|index| u16::try_from(index).ok())
            .map(BuildingKind)
    };
    loop {
        let added: Vec<BuildingKind> = kinds()
            .filter(|kind| !reached.contains(kind))
            .filter(|&kind| {
                let item = ProductionItem::Building(kind);
                within_reach(sim::check_buildable(
                    definitions,
                    state,
                    superweapons,
                    &reached,
                    item,
                ))
            })
            .collect();
        if added.is_empty() {
            return reached;
        }
        reached.extend(added);
    }
}

/// Whether the outcome `check` of `sim::check_buildable` means that the item belongs to what the
/// player can build: it may be queued, or only its build limit keeps it from being queued again
/// (RA2: the card of a building the player owns as often as it may stays listed).
fn within_reach(check: Result<(), sim::RejectReason>) -> bool {
    matches!(
        check,
        Ok(())
            | Err(sim::RejectReason::BuildLimitReached { .. })
            | Err(sim::RejectReason::NoFreeDock(_))
    )
}

/// Whether the card of `item` is listed for a player with `state` who owns buildings of the
/// types `owned`, can come to own those of `reachable` ([`reachable_buildings`]) and has `queued`
/// entries of the item in a queue (RA1; Red Alert 2 shows a cameo for what a player's factories
/// can build):
///
/// * some owned building is a factory of the item's kind and of a side that builds it
///   (`sim::produces`): an Allied player sees no Soviet unit, sees the Conscript once it owns a
///   captured Soviet Barracks, and sees no infantry at all before it owns a barracks;
/// * the player could queue the item if it owned every building it can come to own by building
///   alone (`sim::check_buildable` with `reachable`): the item is not closed to the player's
///   country, nor outside the tech level of the rules or the roster of a faction without a
///   side, nor a superweapon building in a match with superweapons off (`superweapons`), and
///   every prerequisite is owned or can be built. This is what keeps the other side's
///   buildings away: the ini lets every country own every building (`Owner`) and ties each to
///   a side by a prerequisite, the Construction Yard of that side or a building that needs it,
///   which a player of the other side can only capture.
///
/// So an item whose prerequisites are missing but within the player's reach is listed, and
/// shown as locked with the reason ([`availability_text`]). An item that is queued stays listed
/// whatever became of its factory, so that its progress shows and it can be cancelled.
pub fn item_listed(
    item: ProductionItem,
    definitions: &Definitions,
    owned: &BTreeSet<BuildingKind>,
    reachable: &[BuildingKind],
    state: &PlayerState,
    superweapons: bool,
    queued: usize,
) -> bool {
    queued > 0
        || (owned
            .iter()
            .any(|&kind| sim::produces(definitions, kind, item))
            && within_reach(sim::check_buildable(
                definitions,
                state,
                superweapons,
                reachable,
                item,
            )))
}

/// Whether superweapons are switched on in the match of `host` (the menu's row; on without a
/// match).
fn superweapons(host: &SimHost) -> bool {
    host.match_state()
        .is_none_or(|state| state.rules().superweapons)
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
    let superweapons = superweapons(host);
    production_menu(definitions)
        .into_iter()
        .map(|item| {
            (
                item,
                item_status(item, definitions, &owned, state, superweapons),
            )
        })
        .collect()
}

/// The production items whose cards the sidebar lists for the human player of `host`'s match
/// on any tab ([`item_shown`] decides the tab, [`item_listed`] the rest). Every item without a
/// match.
pub fn listed_items(host: &SimHost) -> Vec<ProductionItem> {
    let Some(definitions) = host.definitions() else {
        return Vec::new();
    };
    let menu = production_menu(definitions);
    let Some(state) = host.players().and_then(|players| players.get(HUMAN)) else {
        return menu;
    };
    let owned = owned_kinds(host);
    let superweapons = superweapons(host);
    let reachable = reachable_buildings(definitions, &owned, state, superweapons);
    menu.into_iter()
        .filter(|&item| {
            let queued = item_status(item, definitions, &owned, state, superweapons).queued;
            state.offers(item)
                && item_listed(
                    item,
                    definitions,
                    &owned,
                    &reachable,
                    state,
                    superweapons,
                    queued,
                )
        })
        .collect()
}

/// What the details line says while no card is hovered: the keys, or, while the player owns no
/// building and so no card is listed (`base` false), how to get the first.
pub fn details_hint(base: bool) -> &'static str {
    if base {
        "Hover: requirements / role\nSpace + drag: camera | D deploy\nRight click card: cancel / refund"
    } else {
        "No Construction Yard: nothing to build.\nSelect your MCV and press D\nto deploy it where it stands."
    }
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
                    // The panel of a minimap without radar (RA2, `crate::minimap`): empty
                    // while the player has radar.
                    minimap.spawn((
                        SidebarText::Radar,
                        text("", 13.0),
                        TextLayout::justify(Justify::Center),
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(0.0),
                            right: Val::Px(0.0),
                            top: Val::Px(38.0),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
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
                            .with_children(|tab_button| {
                                tab_button.spawn(text(label, 11.0));
                                // The progress of the tab's lane, along the button's lower edge.
                                tab_button.spawn((
                                    ItemProgress::Tab(tab),
                                    Node {
                                        position_type: PositionType::Absolute,
                                        left: Val::Px(0.0),
                                        bottom: Val::Px(0.0),
                                        width: Val::Percent(0.0),
                                        height: Val::Px(3.0),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.95, 0.72, 0.22)),
                                ));
                            });
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
                            line.spawn((SidebarText::Cost(item), text(cost, 11.0)))
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
                            ItemProgress::Item(item),
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
            Hotkey::Tab(n) => SidebarEffect::Tab(
                [Tab::Buildings, Tab::Armory, Tab::Infantry, Tab::Vehicles][usize::from(n).min(3)],
            ),
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
    // The cards of the tab that the player's buildings could build (RA1, `item_listed`).
    let all = listed_items(&host);
    let listed: Vec<_> = defs
        .map(|d| {
            all.iter()
                .copied()
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
            SidebarText::PageLabel => format!("{} / {}  |  Tab cycles", page.0 + 1, pages),
            SidebarText::Details => hovered
                .and_then(|i| {
                    defs.map(|d| {
                        let description = d.metadata(i).map_or("", |e| e.description.as_str());
                        format!(
                            "{}\n{}",
                            availability_text(i, d, &owned, state, factories_of(&host, i)),
                            description
                        )
                    })
                })
                .unwrap_or_else(|| details_hint(state.is_none() || !owned.is_empty()).into()),
            SidebarText::Cost(item) => defs
                .and_then(|defs| defs.production(*item))
                .map(|terms| {
                    format!(
                        "${}",
                        state.map_or(terms.cost, |state| state.production_cost(terms.cost))
                    )
                })
                .unwrap_or_default(),
            SidebarText::Credits => credits_text(state),
            SidebarText::Power => power_text(state),
            SidebarText::Mode => mode_text(*mode, paused, host.definitions()),
            // Queued progress stays visible even if a prerequisite was lost meanwhile.
            SidebarText::Badge(item) => {
                let status = status_of(*item);
                if status.available || status.ready || status.queued > 0 || status.limited {
                    badge_text(status)
                } else {
                    "LOCKED".into()
                }
            }
            SidebarText::Volume(channel) => volume_text(&volume, *channel),
            SidebarText::PauseLabel => if paused { "Resume" } else { "Pause" }.into(),
            SidebarText::MuteLabel => if volume.muted { "Unmute" } else { "Mute" }.into(),
            SidebarText::Speed => format!("Speed {}", speed.label()),
            SidebarText::Radar => crate::minimap::offline_text(&host).unwrap_or("").into(),
        };
        if text.0 != value {
            text.0 = value;
        }
        let wanted = match kind {
            SidebarText::Cost(_) => Color::srgb(0.89, 0.78, 0.51),
            SidebarText::Power if state.is_some_and(PlayerState::low_power) => {
                Color::srgb(1.0, 0.36, 0.26)
            }
            SidebarText::Power => Color::srgb(0.55, 0.92, 0.42),
            SidebarText::Credits | SidebarText::Radar => Color::srgb(1.0, 0.80, 0.32),
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
        let percent = match bar {
            ItemProgress::Item(item) => progress_percent(status_of(*item)),
            ItemProgress::Tab(tab) => defs
                .and_then(|defs| tab_progress(*tab, &statuses, defs))
                .unwrap_or(0.0),
        };
        let width = Val::Percent(percent);
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
        // Two power plants ordered through the sink's path: the head progresses. Since RA2 the
        // lane of the buildings holds one item, so the second order is rejected and one is
        // queued (before: both, and the badge said "50% +1").
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
        assert_eq!(half.queued, 1);
        assert_eq!(half.progress, Some((total / 2, total)));
        assert!(!half.ready);
        assert_eq!(badge_text(half), "50%");
        assert_eq!(progress_percent(half), 50.0);
        // The lane is taken: its other cards say so and a click on one orders nothing; the
        // tab's bar shows the lane's progress, and the lane of the defences is free.
        assert!(half.full);
        let barracks =
            ProductionItem::Building(definitions.building_kind("barracks").expect("barracks"));
        // (The barracks still needs the power plant; a card that is available but for the
        // taken lane says "busy".)
        let locked = status(&host, barracks);
        assert!(!locked.available && locked.full && locked.queued == 0);
        assert_eq!(badge_text(locked), "locked");
        let busy = ItemStatus {
            available: true,
            ..locked
        };
        assert_eq!(badge_text(busy), "busy");
        let menu = production_menu(&definitions);
        assert_eq!(
            button_effect(
                SidebarButton::Item(barracks),
                MouseButton::Left,
                &menu,
                busy,
                InterfaceMode::Normal
            ),
            SidebarEffect::Nothing
        );
        let all = statuses(&host);
        assert_eq!(tab_progress(Tab::Buildings, &all, &definitions), Some(50.0));
        assert_eq!(tab_progress(Tab::Armory, &all, &definitions), None);
        assert_eq!(tab_progress(Tab::Vehicles, &all, &definitions), None);
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

    /// RA4: a Harrier whose docks are all taken is not available, stays listed like a card at
    /// its build limit, and says why; with a second Airforce Command of the same type it is
    /// available again, although the sidebar lists the type once.
    #[test]
    fn an_aircraft_without_a_free_dock_stays_listed_and_says_so() {
        let spawn = |commands: usize| {
            crate::sim_host::tests::demo_host_with(move |world| {
                let mut placed = Vec::new();
                for id in ["power_plant", "refinery"]
                    .into_iter()
                    .chain(std::iter::repeat_n("american_airforce_command", commands))
                {
                    let spot = (30..60)
                        .flat_map(|y| (20..60).map(move |x| CellPos::new(x, y)))
                        .find_map(|cell| {
                            sim::spawn_building(world, HUMAN, id, cell)
                                .ok()
                                .map(|_| cell)
                        });
                    placed.push(spot.unwrap_or_else(|| panic!("ground for {id}")));
                }
                // Four Harriers beside the first Airforce Command, which they fly to.
                let at = placed[2];
                for n in 0..4 {
                    sim::spawn_unit(world, HUMAN, "harrier", CellPos::new(at.x + n, at.y))
                        .expect("a Harrier above its Airforce Command");
                }
                // The simulation counts aircraft and docks in its `Economy` stage.
                world.run_schedule(sim::SimSchedule);
            })
        };
        let harrier = |host: &SimHost| {
            let definitions = host.definitions().expect("definitions");
            ProductionItem::Unit(definitions.unit_kind("harrier").expect("harrier"))
        };
        let status = |host: &SimHost| {
            statuses(host)
                .into_iter()
                .find(|(entry, _)| *entry == harrier(host))
                .map(|(_, status)| status)
                .expect("on the menu")
        };
        let full = spawn(1);
        let state = full.players().and_then(|players| players.get(HUMAN));
        assert_eq!(
            state.map(|state| (state.docked_aircraft(), state.docks())),
            Some((4, 4))
        );
        let found = status(&full);
        assert!(!found.available && found.limited);
        assert!(listed_items(&full).contains(&harrier(&full)));
        assert_eq!(
            availability_text(
                harrier(&full),
                full.definitions().expect("definitions"),
                &owned_kinds(&full),
                state,
                1
            ),
            "No free dock: every dock is taken"
        );
        // Eight docks on two buildings of one type: four are free.
        let roomy = spawn(2);
        let state = roomy.players().and_then(|players| players.get(HUMAN));
        assert_eq!(
            state.map(|state| (state.docked_aircraft(), state.docks())),
            Some((4, 8))
        );
        assert_eq!(owned_kinds(&roomy).len(), owned_kinds(&full).len());
        let found = status(&roomy);
        assert!(found.available && !found.limited);
    }

    #[test]
    fn a_card_at_its_build_limit_stays_listed_and_says_so() {
        // RA2: one Ore Purifier per player. The human player of the demo match owns its yard;
        // here also a Battle Lab and a refinery (the purifier's prerequisites) and, in the
        // second host, the purifier itself.
        let spawn = |ids: &'static [&'static str]| {
            crate::sim_host::tests::demo_host_with(move |world| {
                for id in ids {
                    let spot = (30..60)
                        .flat_map(|y| (20..60).map(move |x| CellPos::new(x, y)))
                        .find(|&cell| sim::spawn_building(world, HUMAN, id, cell).is_ok());
                    assert!(spot.is_some(), "ground for {id}");
                }
            })
        };
        let status = |host: &SimHost, item| {
            statuses(host)
                .into_iter()
                .find(|(entry, _)| *entry == item)
                .map(|(_, status)| status)
                .expect("on the menu")
        };
        let free = spawn(&["tech_center", "refinery"]);
        let definitions = free.definitions().expect("definitions").clone();
        let purifier = ProductionItem::Building(
            definitions
                .building_kind("ore_purifier")
                .expect("ore purifier"),
        );
        let before = status(&free, purifier);
        assert!(before.available && !before.limited);
        assert!(listed_items(&free).contains(&purifier));
        // Owned: not available, at its limit, still listed, and a click orders nothing.
        let owned = spawn(&["tech_center", "refinery", "ore_purifier"]);
        let limited = status(&owned, purifier);
        assert!(!limited.available && limited.limited && limited.queued == 0);
        assert_eq!(badge_text(limited), "LIMIT");
        assert!(listed_items(&owned).contains(&purifier));
        let state = owned.players().and_then(|players| players.get(HUMAN));
        assert_eq!(
            availability_text(purifier, &definitions, &owned_kinds(&owned), state, 1),
            "Build limit reached: 1 per player"
        );
        let menu = production_menu(&definitions);
        assert_eq!(
            button_effect(
                SidebarButton::Item(purifier),
                MouseButton::Left,
                &menu,
                limited,
                InterfaceMode::Normal
            ),
            SidebarEffect::Nothing
        );
    }

    #[test]
    fn the_time_of_an_item_follows_the_factories_and_the_power() {
        // RA2: a Grizzly of 588 ticks takes 30 seconds at one War Factory and 24 at two; a
        // world without a rules file keeps its build time.
        let host = crate::sim_host::tests::demo_host();
        let definitions = host.definitions().expect("definitions");
        let state = host.players().and_then(|players| players.get(HUMAN));
        assert_eq!(build_ticks(588, definitions, state, 1), 588);
        assert_eq!(build_ticks(588, definitions, state, 2), 471);
        assert_eq!(build_ticks(588, definitions, state, 0), 588);
        assert_eq!(build_ticks(588, definitions, None, 3), 377);
        let legacy = crate::skirmish::tests::legacy_catalogue();
        assert_eq!(build_ticks(588, &legacy.definitions, None, 3), 588);
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
                let status = item_status(item, definitions, &owned, state, true);
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

    /// A player of `country` (a faction id of the shipped skirmish file) with nothing but the
    /// country set.
    fn player_of(catalogue: &crate::skirmish::Catalogue, country: &str) -> sim::Players {
        let faction = catalogue
            .skirmish
            .factions
            .iter()
            .find(|faction| faction.id == country)
            .expect(country);
        let mut players = sim::Players::default();
        players.add(HUMAN, 0).expect("player");
        assert!(players.set_country(HUMAN, faction.country));
        players
    }

    #[test]
    fn a_tab_lists_only_what_a_building_of_the_player_could_build() {
        // RA1 item 2: the sidebar lists an item when some building the player owns is a factory
        // of the item's kind and of a side that builds it, and the item is not closed to the
        // player's country; a missing prerequisite locks the card and does not hide it.
        let catalogue = crate::skirmish::tests::production_catalogue();
        let defs = &catalogue.definitions;
        let menu = production_menu(defs);
        let building = |id: &str| defs.building_kind(id).unwrap_or_else(|| panic!("{id}"));
        let item = |id: &str| match defs.unit_kind(id) {
            Some(kind) => ProductionItem::Unit(kind),
            None => ProductionItem::Building(building(id)),
        };
        let owned = |ids: &[&str]| -> BTreeSet<BuildingKind> {
            ids.iter().map(|id| building(id)).collect()
        };
        let america = player_of(&catalogue, "america");
        let america = america.get(HUMAN).expect("player");
        // Whether the card of `item` is listed for a player who owns `owned`, with
        // superweapons on or off and `queued` entries of it queued.
        let card = |state: &PlayerState,
                    owned: &BTreeSet<BuildingKind>,
                    item: ProductionItem,
                    superweapons: bool,
                    queued: usize| {
            let reachable = reachable_buildings(defs, owned, state, superweapons);
            item_listed(item, defs, owned, &reachable, state, superweapons, queued)
        };
        let listed = |state: &PlayerState, owned: &BTreeSet<BuildingKind>, id: &str| {
            card(state, owned, item(id), true, 0)
        };
        let sides =
            |item: ProductionItem| defs.metadata(item).expect("arsenal").owner_sides.clone();

        // Before the MCV has deployed the player owns nothing, and no card is listed.
        let nothing = BTreeSet::new();
        assert!(
            menu.iter()
                .all(|&item| !card(america, &nothing, item, true, 0))
        );
        assert!(reachable_buildings(defs, &nothing, america, true).is_empty());
        // With the Allied yard: buildings of the Allied side only, no unit, no Soviet card.
        let yard = owned(&["construction_yard"]);
        let cards: Vec<ProductionItem> = menu
            .iter()
            .copied()
            .filter(|&item| card(america, &yard, item, true, 0))
            .collect();
        // What the yard can come to build is the tech tree of the country: its buildings are
        // the cards, with the yard itself.
        let america_tree = sim::country_tree(defs, america.country().expect("country"));
        let reach: BTreeSet<BuildingKind> = reachable_buildings(defs, &yard, america, true)
            .into_iter()
            .collect();
        assert_eq!(
            reach,
            america_tree
                .iter()
                .filter_map(|item| match item {
                    ProductionItem::Building(kind) => Some(*kind),
                    ProductionItem::Unit(_) => None,
                })
                .chain(yard.iter().copied())
                .collect()
        );
        assert_eq!(cards.len() + 1, reach.len());
        assert!(cards.len() > 5, "{}", cards.len());
        let soviet_yard = building("soviet_construction_yard");
        for &card in &cards {
            assert!(matches!(card, ProductionItem::Building(_)), "{card:?}");
            // (The ini lets every country own every building; its side is the yard it needs.)
            assert!(sides(card).contains(&sim::Side::Allied), "{card:?}");
            let terms = defs.production(card).expect("terms");
            assert!(!terms.prerequisites.contains(&soviet_yard), "{card:?}");
        }
        assert!(listed(america, &yard, "power_plant"));
        assert!(item_status(item("power_plant"), defs, &yard, america, true).available);
        // Listed and locked, with the reason: the Refinery needs a power plant.
        assert!(listed(america, &yard, "refinery"));
        assert!(!item_status(item("refinery"), defs, &yard, america, true).available);
        assert_eq!(
            availability_text(item("refinery"), defs, &yard, Some(america), 1),
            "Needs Power Plant"
        );
        for soviet in [
            "tesla_reactor",
            "soviet_barracks",
            "soviet_war_factory",
            "tesla_coil",
        ] {
            assert!(!listed(america, &yard, soviet), "{soviet}");
            let terms = defs.production(item(soviet)).expect("terms");
            assert!(terms.prerequisites.contains(&soviet_yard), "{soviet}");
        }
        // The Soviet walls need the Soviet Barracks and no yard: no card either, the barracks
        // being out of an Allied player's reach.
        assert!(!listed(america, &yard, "soviet_walls"));
        assert!(listed(america, &yard, "fortress_walls"));
        assert_eq!(sides(item("conscript")), [sim::Side::Soviet]);
        assert_eq!(sides(item("gi")), [sim::Side::Allied]);
        for unit in ["gi", "conscript", "grizzly", "rhino", "engineer"] {
            assert!(!listed(america, &yard, unit), "{unit}: no factory for it");
        }
        // National specials: America builds its own Airforce Command and not the common one;
        // France alone the Grand Cannon.
        assert!(listed(america, &yard, "american_airforce_command"));
        assert!(!listed(america, &yard, "airforce_command"));
        assert!(!listed(america, &yard, "grand_cannon"));
        let france = player_of(&catalogue, "france");
        let france = france.get(HUMAN).expect("player");
        assert!(listed(france, &yard, "grand_cannon"));
        assert!(listed(france, &yard, "airforce_command"));
        assert!(!listed(france, &yard, "american_airforce_command"));

        // With its own Barracks the Allied infantry is listed, and still nothing Soviet. The
        // Rocketeer needs a radar building, which the player can build: listed and locked.
        let base = owned(&["construction_yard", "power_plant", "barracks"]);
        assert!(listed(america, &base, "gi"));
        assert!(listed(america, &base, "engineer"));
        assert!(listed(america, &base, "rocketeer"));
        assert!(!item_status(item("rocketeer"), defs, &base, america, true).available);
        assert!(
            availability_text(item("rocketeer"), defs, &base, Some(america), 1)
                .starts_with("Needs ")
        );
        assert!(!listed(america, &base, "conscript"));
        assert!(!listed(america, &base, "tesla_trooper"));
        // After capturing a Soviet Barracks the player sees the Conscript and what else that
        // barracks trains by itself, and the Soviet walls, which need nothing but it. The
        // Soviet infantry that needs a building of the Soviet yard stays away (Yuri: the Battle
        // Lab), as do the Soviet buildings and the specials of the Soviet countries.
        let captured = owned(&[
            "construction_yard",
            "power_plant",
            "barracks",
            "soviet_barracks",
        ]);
        assert!(listed(america, &captured, "conscript"));
        assert!(item_status(item("conscript"), defs, &captured, america, true).available);
        assert!(listed(america, &captured, "tesla_trooper"));
        assert!(listed(america, &captured, "soviet_walls"));
        assert!(
            listed(america, &captured, "gi"),
            "its own infantry as before"
        );
        for other in [
            "yuri",
            "flak_trooper",
            "tesla_reactor",
            "soviet_war_factory",
            "rhino",
            "terrorist",
            "desolator",
        ] {
            assert!(!listed(america, &captured, other), "{other}");
        }
        // A captured Soviet Barracks alone lists the Soviet infantry and no Allied one.
        let only = owned(&["soviet_barracks"]);
        assert!(listed(america, &only, "conscript"));
        assert!(!listed(america, &only, "gi"));
        assert!(listed(america, &only, "engineer"), "any barracks trains it");
        // A Soviet player is the mirror image.
        let cuba = player_of(&catalogue, "cuba");
        let cuba = cuba.get(HUMAN).expect("player");
        let soviet_base = owned(&[
            "soviet_construction_yard",
            "tesla_reactor",
            "soviet_barracks",
        ]);
        assert!(listed(cuba, &soviet_base, "conscript"));
        assert!(listed(cuba, &soviet_base, "terrorist"), "Cuba's own");
        assert!(listed(cuba, &soviet_base, "tesla_reactor"));
        assert!(!listed(cuba, &soviet_base, "desolator"), "Iraq's");
        for allied in ["gi", "power_plant", "barracks", "pillbox"] {
            assert!(!listed(cuba, &soviet_base, allied), "{allied}");
        }

        // Superweapons off: their buildings are not listed, whatever the player owns.
        let everything: BTreeSet<BuildingKind> = (0..defs.buildings().len())
            .map(|index| BuildingKind(index as u16))
            .collect();
        let superweapons: Vec<ProductionItem> = menu
            .iter()
            .copied()
            .filter(|&item| {
                matches!(item, ProductionItem::Building(_))
                    && defs
                        .metadata(item)
                        .is_some_and(|entry| entry.role.major_superweapon())
            })
            .collect();
        assert!(superweapons.len() >= 4, "{superweapons:?}");
        for &weapon in &superweapons {
            assert!(card(america, &everything, weapon, true, 0), "{weapon:?}");
            assert!(!card(america, &everything, weapon, false, 0), "{weapon:?}");
            assert!(!item_status(weapon, defs, &everything, america, false).available);
        }
        // Nothing else changes with the option.
        for &other in menu.iter().filter(|item| !superweapons.contains(item)) {
            assert_eq!(
                card(america, &everything, other, true, 0),
                card(america, &everything, other, false, 0),
                "{other:?}"
            );
        }
        // The Construction Yards and the units a spy unlocks are no cards at all.
        for never in [
            "construction_yard",
            "soviet_construction_yard",
            "chrono_ivan",
        ] {
            assert!(!listed(america, &everything, never), "{never}");
        }
        // An item in a queue stays listed when its factory is gone, so that it can be cancelled.
        assert!(!listed(america, &nothing, "conscript"));
        assert!(card(america, &nothing, item("conscript"), true, 1));

        // Through the simulation: the match as the menu starts it lists no card (the player
        // owns its MCV and no building); with the yard it lists the Allied buildings; and a
        // Soviet Barracks in the player's hands adds the Conscript.
        let ids = |host: &SimHost| -> BTreeSet<ProductionItem> {
            listed_items(host).into_iter().collect()
        };
        assert!(listed_items(&crate::sim_host::tests::menu_host()).is_empty());
        assert_eq!(details_hint(false).lines().count(), 3);
        assert!(details_hint(false).contains("press D"));
        assert_ne!(details_hint(true), details_hint(false));
        let host = crate::sim_host::tests::demo_host();
        assert_eq!(ids(&host), cards.iter().copied().collect());
        let mut placed = None;
        let host = crate::sim_host::tests::demo_host_with(|world| {
            // The first free site beside the human player's base.
            placed = (30..44)
                .flat_map(|y| (6..24).map(move |x| CellPos::new(x, y)))
                .find_map(|cell| sim::spawn_building(world, HUMAN, "soviet_barracks", cell).ok());
        });
        placed.expect("a site for the barracks");
        let with_barracks = ids(&host);
        assert!(with_barracks.contains(&item("conscript")));
        assert!(with_barracks.contains(&item("engineer")));
        assert!(!with_barracks.contains(&item("gi")));
        assert!(!with_barracks.contains(&item("tesla_reactor")));
        assert!(with_barracks.is_superset(&cards.iter().copied().collect()));
        // Without a match every item is listed (the sidebar is not shown then).
        let empty = crate::sim_host::tests::fixture_host(|_| {});
        assert_eq!(
            listed_items(&empty).len(),
            production_menu(empty.definitions().expect("definitions")).len()
        );
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
        assert!(!item_status(coil, defs, &owned, &state, true).available);
        assert!(availability_text(coil, defs, &owned, Some(&state), 1).starts_with("Needs"));
        let unlocked = {
            let mut app = App::new();
            app.add_plugins(sim::SimPlugin { seed: 0 });
            app.world_mut()
                .insert_resource(catalogue.maps[0].map.grid.clone());
            app.world_mut()
                .insert_resource(catalogue.definitions.clone());
            let selection = crate::skirmish::MenuSelection::versus(&catalogue);
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
        assert!(item_status(coil, defs, &owned, &state, true).available);
        assert!(
            !item_status(tank, defs, &owned, &state, true).available,
            "needs a war factory"
        );
        assert_eq!(
            availability_text(tank, defs, &owned, Some(&state), 1),
            "Needs War Factory"
        );
        // Progress of a queued item stays visible even while it is not available.
        let queued = ItemStatus {
            available: false,
            queued: 1,
            progress: Some((5, 10)),
            ready: false,
            limited: false,
            full: false,
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

    // Append inside sidebar::tests. Actual persistent ItemList, teardown, install and systems.
    fn ra9_boundary_sidebar_app(host: SimHost) -> App {
        let mut app = App::new();
        app.insert_resource(host)
            .init_resource::<Assets<Image>>()
            .init_resource::<CommandSink>()
            .init_resource::<Tab>()
            .init_resource::<BuildPage>()
            .init_resource::<InterfaceMode>()
            .init_resource::<VolumeSettings>()
            .init_resource::<crate::game_speed::GameSpeed>()
            .insert_resource(crate::skirmish::Screen::Match)
            .add_systems(Update, (populate_items, update_sidebar).chain());
        app.world_mut().spawn((ItemList, Node::default()));
        app
    }
    fn ra9_boundary_card(world: &mut World, item: ProductionItem) -> (Entity, String, Color) {
        let card = world
            .query::<(Entity, &SidebarButton)>()
            .iter(world)
            .find_map(|(entity, button)| (*button == SidebarButton::Item(item)).then_some(entity))
            .expect("actual card");
        let mut todo = vec![card];
        while let Some(entity) = todo.pop() {
            if let (Some(text), Some(colour)) =
                (world.get::<Text>(entity), world.get::<TextColor>(entity))
                && text.0.starts_with('$')
            {
                return (card, text.0.clone(), colour.0);
            }
            if let Some(children) = world.get::<Children>(entity) {
                todo.extend(children.iter());
            }
        }
        panic!("native populated card missing cost text")
    }
    #[test]
    fn ra9_cost_card_updates_across_real_teardown_and_successive_installs() {
        let catalogue = crate::skirmish::tests::production_catalogue();
        let item = ProductionItem::Building(
            catalogue
                .definitions
                .building_kind("power_plant")
                .expect("power"),
        );
        let base = catalogue.definitions.production(item).expect("terms").cost;
        let mut app = ra9_boundary_sidebar_app(crate::sim_host::tests::demo_host());
        let mut previous = None;
        for super_me in [false, true, false] {
            crate::skirmish::end_match(app.world_mut());
            let selection = crate::skirmish::MenuSelection {
                super_me,
                ..crate::skirmish::MenuSelection::versus(&catalogue)
            };
            let (plan, _) =
                crate::skirmish::match_plan(&catalogue, selection, 7).expect("real plan");
            crate::sim_host::install_match(app.world_mut(), &plan).expect("real install");
            app.world_mut().run_schedule(Update);
            let (card, cost, colour) = ra9_boundary_card(app.world_mut(), item);
            assert_eq!(cost, format!("${}", if super_me { 1 } else { base }));
            assert_eq!(
                colour,
                Color::srgb(0.89, 0.78, 0.51),
                "original gold cost styling"
            );
            if let Some(previous) = previous {
                assert_eq!(
                    card, previous,
                    "actual persistent card, no reconstruction hides stale text"
                );
            }
            previous = Some(card);
        }
    }

    fn ra9_factory_boundary_host(disabled: usize, dead_second: bool) -> (SimHost, ProductionItem) {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data");
        let defs = sim::load_ruleset(root).expect("shipped rules");
        let item = ProductionItem::Unit(defs.unit_kind("grizzly").expect("grizzly"));
        let country = defs
            .rules()
            .expect("rules")
            .country_id("america")
            .expect("country");
        let host = crate::sim_host::tests::fixture_host(|world| {
            world.insert_resource(
                sim::Grid::from_cells(
                    32,
                    32,
                    vec![
                        sim::Cell {
                            passable: true,
                            water: false,
                            buildable: true,
                            ore: 0
                        };
                        1024
                    ],
                )
                .expect("grid"),
            );
            world.insert_resource(defs);
            for player in [HUMAN, sim::PlayerId(1)] {
                world
                    .resource_mut::<sim::Players>()
                    .add(player, 10_000)
                    .expect("player");
            }
            assert!(
                world
                    .resource_mut::<sim::Players>()
                    .set_country(HUMAN, Some(country))
            );
            for (kind, cell) in [
                ("construction_yard", CellPos::new(8, 8)),
                ("nuclear_reactor", CellPos::new(2, 2)),
                ("barracks", CellPos::new(8, 12)),
                ("refinery", CellPos::new(3, 12)),
            ] {
                sim::spawn_building(world, HUMAN, kind, cell).expect("scenario setup");
            }
            for (index, cell) in [CellPos::new(12, 12), CellPos::new(18, 12)]
                .into_iter()
                .enumerate()
            {
                let id = sim::spawn_building(world, HUMAN, "war_factory", cell).expect("factory");
                if index < disabled {
                    world
                        .resource_mut::<sim::SpecialState>()
                        .disabled
                        .insert(id, 20);
                }
                if index == 1 && dead_second {
                    let entity = world
                        .query::<(Entity, &sim::SimId)>()
                        .iter(world)
                        .find_map(|(e, found)| (*found == id).then_some(e))
                        .expect("factory entity");
                    world
                        .get_mut::<sim::Health>(entity)
                        .expect("health")
                        .hit_points = 0; // Setup at a real pre-Cleanup stage boundary.
                }
            }
            sim::spawn_building(world, sim::PlayerId(1), "war_factory", CellPos::new(26, 24))
                .expect("foreign factory control");
        });
        (host, item)
    }
    fn ra9_factory_boundary_ui(host: SimHost, item: ProductionItem) -> (App, Entity) {
        let mut app = ra9_boundary_sidebar_app(host);
        *app.world_mut().resource_mut::<Tab>() = Tab::Vehicles;
        let details = app
            .world_mut()
            .spawn((SidebarText::Details, Text::new(""), TextColor::default()))
            .id();
        app.world_mut().spawn((
            SidebarButton::Item(item),
            Interaction::Hovered,
            Node::default(),
            BackgroundColor::default(),
        ));
        // Do not also populate a duplicate item card for this hover probe.
        let lists = app
            .world_mut()
            .query_filtered::<Entity, With<ItemList>>()
            .iter(app.world())
            .collect::<Vec<_>>();
        for list in lists {
            app.world_mut().despawn(list);
        }
        (app, details)
    }
    fn ra9_factory_progress(host: &SimHost, item: ProductionItem) -> u32 {
        let key = host
            .definitions()
            .expect("definitions")
            .queue_of(item)
            .expect("queue");
        host.players()
            .expect("players")
            .get(HUMAN)
            .expect("human")
            .lane(key)
            .first()
            .expect("active queue")
            .progress
    }
    #[test]
    fn ra9_factory_hint_matches_actual_disabled_expiry_production_and_foreign_control() {
        let (mut host, item) = ra9_factory_boundary_host(1, false);
        crate::sim_host::tests::run(&mut host, vec![]); // Actual power update and disabled stage.
        assert_eq!(factories_of(&host, item), 1);
        let (mut app, details) = ra9_factory_boundary_ui(host, item);
        app.world_mut().run_schedule(Update);
        assert!(
            app.world()
                .get::<Text>(details)
                .expect("details")
                .0
                .starts_with("30 seconds")
        );
        let mut host = app.world_mut().remove_resource::<SimHost>().expect("host");
        let events =
            crate::sim_host::tests::run(&mut host, vec![Command::QueueProduction { item }]);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, sim::SimEvent::CommandRejected { .. })),
            "{events:?}"
        );
        for _ in 0..3 {
            crate::sim_host::tests::run(&mut host, vec![]);
        }
        assert_eq!(
            ra9_factory_progress(&host, item),
            4,
            "one actual working factory"
        );
        while host.tick() <= 20 {
            crate::sim_host::tests::run(&mut host, vec![]);
        }
        assert_eq!(
            factories_of(&host, item),
            2,
            "actual Economy disabled expiry, never foreign third factory"
        );
        let before = ra9_factory_progress(&host, item);
        for _ in 0..4 {
            crate::sim_host::tests::run(&mut host, vec![]);
        }
        assert_eq!(
            ra9_factory_progress(&host, item) - before,
            5,
            "actual 1.25x progress over four ticks"
        );
        // Cancel the genuine queue to display the same item's unqueued estimate again.
        let events =
            crate::sim_host::tests::run(&mut host, vec![Command::CancelProduction { item }]);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, sim::SimEvent::ProductionCancelled { .. }))
        );
        app.world_mut().insert_resource(host);
        app.world_mut().run_schedule(Update);
        assert!(
            app.world()
                .get::<Text>(details)
                .expect("details")
                .0
                .starts_with("24 seconds")
        );
    }
    #[test]
    fn ra9_zero_working_factory_is_paused_and_dead_sources_cannot_supply_a_rate() {
        let (mut host, item) = ra9_factory_boundary_host(2, false);
        crate::sim_host::tests::run(&mut host, vec![]);
        assert_eq!(factories_of(&host, item), 0);
        let (mut app, details) = ra9_factory_boundary_ui(host, item);
        app.world_mut().run_schedule(Update);
        assert!(
            app.world()
                .get::<Text>(details)
                .expect("details")
                .0
                .starts_with("Production paused: no working factory")
        );
        let mut host = app.world_mut().remove_resource::<SimHost>().expect("host");
        let events =
            crate::sim_host::tests::run(&mut host, vec![Command::QueueProduction { item }]);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, sim::SimEvent::CommandRejected { .. })),
            "queue policy stays native: {events:?}"
        );
        assert_eq!(
            ra9_factory_progress(&host, item),
            0,
            "accepted native queue makes no progress without an exit"
        );
        let (mut host, item) = ra9_factory_boundary_host(0, true);
        assert_eq!(
            factories_of(&host, item),
            1,
            "pre-Cleanup dead Health is not a UI source"
        );
        let events =
            crate::sim_host::tests::run(&mut host, vec![Command::QueueProduction { item }]);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, sim::SimEvent::CommandRejected { .. })),
            "{events:?}"
        );
        let key = host
            .definitions()
            .expect("definitions")
            .queue_of(item)
            .expect("queue");
        assert_eq!(
            host.players()
                .expect("players")
                .get(HUMAN)
                .expect("human")
                .lane(key)[0]
                .fraction,
            0,
            "actual first-tick source liveness: no fractional 250 from the dead second factory before Cleanup"
        );
        for _ in 0..3 {
            crate::sim_host::tests::run(&mut host, vec![]);
        }
        assert_eq!(
            ra9_factory_progress(&host, item),
            4,
            "S alive-source Economy predicate required"
        );
        assert_eq!(factories_of(&host, item), 1);
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sim/tests/fixtures/data");
        let defs = sim::load_definitions(root.join("units_economy.ron"), root.join("economy.ron"))
            .expect("health-less fixture");
        let item = ProductionItem::Unit(defs.unit_kind("walker").expect("walker"));
        let host = crate::sim_host::tests::fixture_host(|world| {
            world.insert_resource(defs);
            world
                .resource_mut::<sim::Players>()
                .add(HUMAN, 1000)
                .expect("player");
            sim::spawn_building(world, HUMAN, "barracks", CellPos::new(4, 4))
                .expect("health-less first");
            sim::spawn_building(world, HUMAN, "barracks", CellPos::new(8, 4))
                .expect("health-less second");
        });
        assert_eq!(
            factories_of(&host, item),
            2,
            "health-less legacy setups remain valid"
        );
    }
}
