//! Selection and orders (PLAN.md section 7, M2): selecting the player's units by click, by
//! dragging a box, and additively with Shift, and ordering the selection to move (right click on
//! a cell) or to stop (S). From M3, a right click on ore sends the selected harvesters to
//! harvest there. From M4, a right click on a visible enemy unit or building orders the selected
//! armed units to attack it, and `A` orders the selection to attack-move to the cell under the
//! cursor (interim keys until the hotkeys of M5; docs/DECISIONS.md D31).
//!
//! From M5 the keys are those of `crate::hotkeys`, control groups recall and store selections,
//! and the map ignores the mouse while the cursor is over the sidebar or the interface is in a
//! mode of `crate::construction` (placing, selling, setting a rally point), whose left and right
//! clicks belong to that mode.
//!
//! A left click on one of the player's unit-producing buildings (barracks, war factory, shipyard,
//! air base) selects it instead ([`SelectedBuilding`]): its rally point is shown as a flag, and a
//! right click on the map then makes the cell under the cursor that building's rally point
//! (`SetRallyPoint`), where every unit it produces from then on goes. `Y` and the sidebar's
//! Rally button still set the rally point of every producer at once (`crate::construction`).
//! A left click on the player's Construction Yard selects it too (RA1): it has no rally point,
//! and with "MCV repacks" on `D` or the Repack button packs it up (`crate::special_controls`).
//!
//! The interface changes the simulation only by pushing commands into `CommandQueue`, through
//! `crate::sim_host::CommandSink` (invariant I5), and reads it through `SimHost` (D34). The
//! decisions are pure functions, unit tested below; the systems only read input and apply them.

use std::collections::BTreeSet;

use bevy::camera::CameraUpdateSystems;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use bevy::window::PrimaryWindow;
use sim::{Building, CellPos, Command, Harvester, Owner, Producer, SimId, Weapon};

use crate::construction::InterfaceMode;
use crate::hd_art::UNIT_GROUND_OFFSET as GROUND_OFFSET_PX;
use crate::hotkeys::{ControlGroups, Hotkey, pressed_hotkeys};
use crate::map::{HoveredCell, MapTerrain, MapView};
use crate::projection::cell_corners;
use crate::sidebar::over_sidebar;
use crate::sim_host::{CommandSink, SimHost};
use crate::units::{HUMAN, UnitView, UnitViews};

/// A press and release of the left button closer together than this (logical pixels) is a
/// click; farther apart, it is a box selection.
pub const DRAG_THRESHOLD_PX: f32 = 4.0;

/// Half the size of the area around a unit's body that a click selects, in world units. The
/// body's centre lies [`GROUND_OFFSET_PX`] above the ground point.
pub const PICK_HALF_SIZE: Vec2 = Vec2::new(20.0, 16.0);

pub struct SelectionPlugin;

impl Plugin for SelectionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Selection>()
            .init_resource::<SelectedBuilding>()
            .init_resource::<DragBox>()
            .init_resource::<ControlGroups>()
            .init_resource::<OrderFeedback>()
            .init_resource::<OrderPreview>()
            .add_systems(
                Startup,
                spawn_intent_label.after(crate::skirmish::load_catalogue),
            )
            .add_systems(
                PostUpdate,
                (
                    prune_selection,
                    prune_selected_building,
                    control_groups,
                    select_units,
                    preview_order,
                    show_intent_cursor,
                    issue_orders,
                    draw_selection,
                )
                    .chain()
                    .after(TransformSystems::Propagate)
                    .after(CameraUpdateSystems)
                    // A right click orders the selection to the cell under the cursor in this
                    // frame (M2 review 1, P2-3).
                    .after(crate::map::update_hovered_cell),
            );
    }
}

/// The player's selected units.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct Selection(pub BTreeSet<SimId>);

/// The player's selected building, if any: a unit-producing building, whose rally point a right
/// click sets, or a Construction Yard, which can be packed up (`crate::special_controls`).
/// Selecting units deselects it, and it deselects the units.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SelectedBuilding(pub Option<SimId>);

/// The cursor position (logical pixels) where the left button went down, while it is held.
#[derive(Resource, Debug, Default)]
pub struct DragBox {
    pub start: Option<Vec2>,
    pub last: Option<Vec2>,
}

#[derive(Resource, Default)]
pub struct OrderFeedback {
    cell: Option<CellPos>,
    until: f64,
}

/// What a right click on the map would order the selection to do, shown before the click by the
/// cursor, a label beside it and a marker on the target.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OrderIntent {
    /// Nothing selected, the cursor is off the map or over the sidebar, or a mode of
    /// `crate::construction` owns the clicks.
    #[default]
    None,
    /// Move to the cell.
    Move,
    /// Attack a visible enemy unit or building (red).
    Attack,
    /// Harvesters gather the ore of the cell.
    Harvest,
    /// The cursor is on one of the player's own units or buildings: the selection moves to it.
    Friendly,
    /// Nothing would happen: outside the map, or terrain none of the selected units can enter.
    Invalid,
    /// A unit-producing building is selected: the cell becomes its rally point.
    Rally,
    Interact(&'static str),
    ForceFire,
    ForceMove,
    Waypoint,
    AttackMove,
}

impl OrderIntent {
    /// The colour of the cursor marker and label.
    pub fn colour(self) -> Color {
        match self {
            Self::None => Color::NONE,
            Self::Move => Color::srgb(0.35, 1.0, 0.4),
            Self::Attack | Self::ForceFire | Self::AttackMove => Color::srgb(1.0, 0.12, 0.08),
            Self::Harvest => Color::srgb(1.0, 0.82, 0.25),
            Self::Friendly => Color::srgb(0.35, 0.8, 1.0),
            Self::Invalid => Color::srgb(0.62, 0.62, 0.62),
            Self::Rally => RALLY_COLOUR,
            Self::Interact(_) | Self::ForceMove | Self::Waypoint => Color::srgb(0.4, 0.85, 1.0),
        }
    }

    /// The label beside the cursor.
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Move => "MOVE",
            Self::Attack => "ATTACK",
            Self::Harvest => "HARVEST",
            Self::Friendly => "FRIENDLY",
            Self::Invalid => "CANNOT GO",
            Self::Rally => "RALLY POINT",
            Self::Interact(label) => label,
            Self::ForceFire => "FORCE FIRE",
            Self::ForceMove => "FORCE MOVE",
            Self::Waypoint => "WAYPOINT",
            Self::AttackMove => "ATTACK MOVE",
        }
    }
}

/// The colour of rally points: the flag, the line to it, and the cursor label.
pub const RALLY_COLOUR: Color = Color::srgb(1.0, 0.6, 0.15);

/// What lies under the cursor, as far as the order decision needs it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HoverFacts {
    /// The cursor is over the map part of the window and the interface is in its normal mode.
    pub over_map: bool,
    /// The number of selected units.
    pub selected: usize,
    /// One of the player's unit-producing buildings is selected (and so no unit is).
    pub producer: bool,
    /// Some selected unit is armed.
    pub armed: bool,
    /// Some selected unit is a harvester.
    pub harvesters: bool,
    /// The visible enemy unit or building under the cursor.
    pub enemy: Option<SimId>,
    /// The player's own unit or building under the cursor.
    pub friendly: Option<SimId>,
    /// The map cell under the cursor (`None` outside the map).
    pub cell: Option<CellPos>,
    /// The cell holds ore (as the map shows it).
    pub ore: bool,
    /// Some selected unit could stand on the cell's terrain, or the player has not explored it.
    pub enterable: bool,
}

/// The order a right click would give ([`OrderIntent`]); the same decision as `issue_orders`.
pub fn order_intent(facts: HoverFacts) -> OrderIntent {
    if !facts.over_map || (facts.selected == 0 && !facts.producer) {
        return OrderIntent::None;
    }
    if facts.cell.is_none() {
        return OrderIntent::Invalid;
    }
    if facts.selected == 0 {
        // Any cell of the map may be a rally point; units go as near to it as they can.
        return OrderIntent::Rally;
    }
    if facts.enemy.is_some() {
        // Unarmed units move next to the target instead.
        return if facts.armed {
            OrderIntent::Attack
        } else {
            OrderIntent::Move
        };
    }
    if facts.friendly.is_some() {
        return OrderIntent::Friendly;
    }
    if facts.ore && facts.harvesters {
        return OrderIntent::Harvest;
    }
    if !facts.enterable {
        return OrderIntent::Invalid;
    }
    OrderIntent::Move
}

/// The order a right click would give now, and its target.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct OrderPreview {
    pub intent: OrderIntent,
    /// The unit or building aimed at (enemy for `Attack`, own for `Friendly`).
    pub target: Option<SimId>,
    /// The cursor in world coordinates.
    pub point: Option<Vec2>,
}

// --- Pure functions -------------------------------------------------------------------------

/// The unit whose body is under `point` (world coordinates), given each selectable unit's
/// ground point: the one whose body centre is nearest, ties broken by the lower id.
pub fn unit_at(point: Vec2, units: &[(SimId, Vec2)]) -> Option<SimId> {
    units
        .iter()
        .filter_map(|&(id, ground)| {
            let body = ground + Vec2::new(0.0, GROUND_OFFSET_PX);
            let offset = (point - body).abs();
            (offset.x <= PICK_HALF_SIZE.x && offset.y <= PICK_HALF_SIZE.y)
                .then(|| (point.distance_squared(body), id))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, id)| id)
}

/// The units whose ground point lies in the box with corners `a` and `b` (world coordinates,
/// edges included).
pub fn units_in_box(a: Vec2, b: Vec2, units: &[(SimId, Vec2)]) -> BTreeSet<SimId> {
    let rect = Rect::from_corners(a, b);
    units
        .iter()
        .filter(|(_, ground)| rect.contains(*ground))
        .map(|(id, _)| *id)
        .collect()
}

/// The selection after picking `picked`: the picked units, or with `additive` (Shift) the
/// current selection plus the picked units.
pub fn apply_selection(
    current: &BTreeSet<SimId>,
    picked: BTreeSet<SimId>,
    additive: bool,
) -> BTreeSet<SimId> {
    if additive {
        current.union(&picked).copied().collect()
    } else {
        picked
    }
}

/// What a left click selects: the unit under the cursor (`unit`) if there is one, otherwise the
/// player's unit-producing building under it (`producer`). Returns the units picked and the
/// building selected; a click on neither deselects both (the units only without Shift,
/// [`apply_selection`]).
pub fn click_pick(
    unit: Option<SimId>,
    producer: Option<SimId>,
) -> (BTreeSet<SimId>, Option<SimId>) {
    match unit {
        Some(unit) => (BTreeSet::from([unit]), None),
        None => (BTreeSet::new(), producer),
    }
}

/// The command for a right click on `cell` while the unit-producing `building` is selected: the
/// cell becomes its rally point. Nothing outside the map.
pub fn rally_command(building: Option<SimId>, cell: Option<CellPos>) -> Option<Command> {
    Some(Command::SetRallyPoint {
        building: building?,
        cell: cell?,
    })
}

/// The command for a right click on `target` (`None` outside the map): move the selection
/// there. Nothing is ordered for an empty selection or outside the map.
pub fn move_command(selection: &BTreeSet<SimId>, target: Option<CellPos>) -> Option<Command> {
    let target = target?;
    (!selection.is_empty()).then(|| Command::Move {
        units: selection.iter().copied().collect(),
        target,
    })
}

/// The commands for a right click on `target` (M3): if the cell holds ore, the selected
/// harvesters (`harvesters` lists every harvester of the player) harvest there and the other
/// selected units move there; otherwise the whole selection moves ([`move_command`]).
pub fn right_click_commands(
    selection: &BTreeSet<SimId>,
    harvesters: &BTreeSet<SimId>,
    target: Option<CellPos>,
    ore_at_target: bool,
) -> Vec<Command> {
    let Some(cell) = target else {
        return Vec::new();
    };
    let (gatherers, others): (BTreeSet<SimId>, BTreeSet<SimId>) =
        selection.iter().partition(|id| harvesters.contains(id));
    if !ore_at_target || gatherers.is_empty() {
        return move_command(selection, target).into_iter().collect();
    }
    let mut commands = vec![Command::Harvest {
        units: gatherers.into_iter().collect(),
        target: cell,
    }];
    commands.extend(move_command(&others, target));
    commands
}

/// The enemy a right click at `point` (world coordinates) is aimed at: the visible enemy unit
/// whose body is under the cursor (`enemy_units` lists every enemy unit the player sees, with
/// its ground point), else the visible enemy building covering the cell under the cursor.
pub fn enemy_at(
    point: Vec2,
    enemy_units: &[(SimId, Vec2)],
    building: Option<SimId>,
) -> Option<SimId> {
    unit_at(point, enemy_units).or(building)
}

/// The commands for a right click on the enemy `target` (M4): the selected units that are armed
/// (`armed` lists every armed unit of the player) attack it, and the other selected units move
/// to the cell under the cursor. Nothing for an empty selection.
pub fn attack_commands(
    selection: &BTreeSet<SimId>,
    armed: &BTreeSet<SimId>,
    target: SimId,
    hovered: Option<CellPos>,
) -> Vec<Command> {
    let (fighters, others): (BTreeSet<SimId>, BTreeSet<SimId>) =
        selection.iter().partition(|id| armed.contains(id));
    let mut commands = Vec::new();
    if !fighters.is_empty() {
        commands.push(Command::Attack {
            units: fighters.into_iter().collect(),
            target,
        });
    }
    commands.extend(move_command(&others, hovered));
    commands
}

/// The command for the attack-move key: the selection attack-moves to the cell under the cursor.
/// Nothing outside the map or for an empty selection.
pub fn attack_move_command(
    selection: &BTreeSet<SimId>,
    target: Option<CellPos>,
) -> Option<Command> {
    let target = target?;
    (!selection.is_empty()).then(|| Command::AttackMove {
        units: selection.iter().copied().collect(),
        target,
    })
}

/// The command for the stop key: stop the selection. Nothing for an empty selection.
pub fn stop_command(selection: &BTreeSet<SimId>) -> Option<Command> {
    (!selection.is_empty()).then(|| Command::Stop {
        units: selection.iter().copied().collect(),
    })
}

// --- Systems --------------------------------------------------------------------------------

/// Whether the unit takes no commands because it was launched by another unit (RA4) or
/// is a strategic mission aircraft (RA6). Neither appears in ordinary unit selections.
fn launched(host: &SimHost, id: SimId) -> bool {
    host.world()
        .get_resource::<sim::SpecialState>()
        .is_some_and(|state| {
            state.spawned.contains_key(&id) || state.strategic.flights.contains_key(&id)
        })
}

fn owned_commandable_unit(host: &SimHost, view: &UnitView) -> bool {
    let world = host.world();
    world.get::<SimId>(view.unit) == Some(&view.id)
        && world
            .get::<Owner>(view.unit)
            .is_some_and(|owner| owner.0 == HUMAN)
        && world
            .get::<sim::Health>(view.unit)
            .is_some_and(|health| health.hit_points > 0)
        && world.get::<sim::Unit>(view.unit).is_some()
        && world.get::<sim::Mobile>(view.unit).is_some()
        && !launched(host, view.id)
}

/// The player's own units that take commands, and their ground points in the world.
fn own_units(host: &SimHost, views: &Query<(&UnitView, &Transform)>) -> Vec<(SimId, Vec2)> {
    views
        .iter()
        .filter(|(view, _)| view.owner == HUMAN && !launched(host, view.id))
        .map(|(view, transform)| {
            (
                view.id,
                transform.translation.truncate() - Vec2::new(0.0, GROUND_OFFSET_PX),
            )
        })
        .collect()
}

fn cursor_world(window: &Window, camera: &Camera, transform: &GlobalTransform) -> Option<Vec2> {
    let cursor = window.cursor_position()?;
    camera.viewport_to_world_2d(transform, cursor).ok()
}

/// Units that disappeared or lost ownership/commandability leave all selection memories.
pub(crate) fn prune_selection(
    host: Res<SimHost>,
    registry: Res<UnitViews>,
    views: Query<&UnitView>,
    mut selection: ResMut<Selection>,
    mut groups: ResMut<ControlGroups>,
) {
    let commandable: BTreeSet<_> = views
        .iter()
        .filter(|view| owned_commandable_unit(&host, view))
        .map(|view| view.id)
        .filter(|id| registry.0.contains_key(id))
        .collect();
    selection.0.retain(|id| commandable.contains(id));
    groups.prune(|id| commandable.contains(&id));
}

/// A selected building that was destroyed, sold or captured is no longer selected, and neither
/// is one while units are selected (control groups and the sidebar select units too).
pub(crate) fn prune_selected_building(
    host: Res<SimHost>,
    selection: Res<Selection>,
    mut building: ResMut<SelectedBuilding>,
) {
    let Some(id) = building.0 else {
        return;
    };
    if !selection.0.is_empty()
        || !(own_producers(&host).contains(&id)
            || own_yards(&host).contains(&id)
            || own_garrisons(&host).contains(&id))
    {
        building.0 = None;
    }
}

pub(crate) fn own_garrisons(host: &SimHost) -> BTreeSet<SimId> {
    host.world()
        .get_resource::<sim::SpecialState>()
        .map(|s| {
            s.garrisons
                .iter()
                .filter(|(_, g)| g.owner == HUMAN)
                .map(|(id, _)| *id)
                .collect()
        })
        .unwrap_or_default()
}

/// The human player's Construction Yards: the buildings that the base unit of a side deploys
/// into and that pack up into it again (the rules file's `sides`). None without a rules file.
pub(crate) fn own_yards(host: &SimHost) -> BTreeSet<SimId> {
    let Some(rules) = host.definitions().and_then(sim::Definitions::rules) else {
        return BTreeSet::new();
    };
    let Some(mut buildings) = host.world().try_query::<(&SimId, &Owner, &Building)>() else {
        return BTreeSet::new();
    };
    buildings
        .iter(host.world())
        .filter(|(_, owner, building)| {
            owner.0 == HUMAN && rules.side_of_construction_yard(building.kind).is_some()
        })
        .map(|(id, ..)| *id)
        .collect()
}

/// The human player's unit-producing buildings.
fn own_producers(host: &SimHost) -> BTreeSet<SimId> {
    let Some(mut buildings) = host
        .world()
        .try_query_filtered::<(&SimId, &Owner), (With<Building>, With<Producer>)>()
    else {
        return BTreeSet::new();
    };
    buildings
        .iter(host.world())
        .filter(|(_, owner)| owner.0 == HUMAN)
        .map(|(id, _)| *id)
        .collect()
}

/// Control group keys (`crate::hotkeys`): select, add, or assign a group.
fn control_groups(
    keys: Res<ButtonInput<KeyCode>>,
    mut groups: ResMut<ControlGroups>,
    mut selection: ResMut<Selection>,
) {
    for action in pressed_hotkeys(&keys) {
        let next = groups.apply(action, &selection.0);
        if next != selection.0 {
            selection.0 = next;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn select_units(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mode: Res<InterfaceMode>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform)>,
    mut drag: ResMut<DragBox>,
    mut selection: ResMut<Selection>,
    mut building: ResMut<SelectedBuilding>,
    hovered: Res<HoveredCell>,
    views: Query<(&UnitView, &Transform)>,
    host: Res<SimHost>,
    time: Res<Time>,
    mut last_click: Local<Option<(SimId, f64)>>,
) {
    let (camera, camera_transform) = *camera;
    if !window.focused
        || keys.pressed(KeyCode::Space)
        || keys.any_pressed([
            KeyCode::ControlLeft,
            KeyCode::ControlRight,
            KeyCode::SuperLeft,
            KeyCode::SuperRight,
            KeyCode::AltLeft,
            KeyCode::AltRight,
            KeyCode::KeyZ,
        ])
        || buttons.pressed(MouseButton::Middle)
    {
        drag.start = None;
        drag.last = None;
        return;
    }
    if drag.start.is_some()
        && let Some(cursor) = window.cursor_position()
    {
        drag.last = Some(cursor.clamp(
            Vec2::ZERO,
            Vec2::new(
                (window.width() - crate::sidebar::SIDEBAR_WIDTH).max(0.0),
                window.height(),
            ),
        ));
    }
    if buttons.just_pressed(MouseButton::Left) {
        // A press over the sidebar, or in a mode whose clicks place, sell, or set a rally
        // point, does not select.
        drag.start = window
            .cursor_position()
            .filter(|&cursor| !over_sidebar(cursor, window.width()))
            .filter(|_| *mode == InterfaceMode::Normal);
        drag.last = drag.start;
    }
    if !buttons.just_released(MouseButton::Left) {
        return;
    }
    let Some(start) = drag.start.take() else {
        return;
    };
    let end = drag.last.take().unwrap_or(start);
    let to_world = |screen: Vec2| camera.viewport_to_world_2d(camera_transform, screen).ok();
    let (Some(start_world), Some(end_world)) = (to_world(start), to_world(end)) else {
        return;
    };
    let units = own_units(&host, &views);
    let mut producer = None;
    let picked = if start.distance(end) < DRAG_THRESHOLD_PX {
        let pick = unit_at(end_world, &units);
        // No unit under the cursor: a click on a factory selects the factory, and one on the
        // Construction Yard the yard.
        producer = click_pick(
            pick,
            own_building(&host, hovered.0).filter(|id| {
                own_producers(&host).contains(id)
                    || own_yards(&host).contains(id)
                    || own_garrisons(&host).contains(id)
            }),
        )
        .1;
        let double = pick.is_some_and(|id| {
            last_click.is_some_and(|(last, at)| last == id && time.elapsed_secs_f64() - at < 0.35)
        });
        *last_click = pick.map(|id| (id, time.elapsed_secs_f64()));
        if double {
            let kind = views
                .iter()
                .find(|(v, _)| Some(v.id) == pick)
                .and_then(|(v, _)| host.world().get::<sim::Unit>(v.unit))
                .map(|u| u.kind);
            views
                .iter()
                .filter(|(v, t)| {
                    v.owner == HUMAN
                        && !launched(&host, v.id)
                        && host
                            .world()
                            .get::<sim::Unit>(v.unit)
                            .is_some_and(|u| Some(u.kind) == kind)
                        && camera
                            .world_to_viewport(camera_transform, t.translation)
                            .is_ok_and(|p| {
                                p.x >= 0.0
                                    && p.y >= 0.0
                                    && p.y < window.height()
                                    && !over_sidebar(p, window.width())
                            })
                })
                .map(|(v, _)| v.id)
                .collect()
        } else {
            pick.into_iter().collect()
        }
    } else {
        units_in_box(
            start_world,
            end_world,
            &units
                .iter()
                .map(|(id, p)| (*id, *p + Vec2::Y * GROUND_OFFSET_PX))
                .collect::<Vec<_>>(),
        )
    };
    let additive = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    // A building is selected alone: it takes the place of the units.
    selection.0 = apply_selection(&selection.0, picked, additive && producer.is_none());
    if building.0 != producer {
        building.0 = producer;
    }
}

/// The enemy units the human player is shown (not hidden by the fog), with their ground points.
fn shown_enemy_units(
    host: &SimHost,
    views: &Query<(&UnitView, &Transform, &Visibility)>,
) -> Vec<(SimId, Vec2)> {
    views
        .iter()
        .filter(|(view, _, visibility)| {
            view.owner != HUMAN
                && **visibility != Visibility::Hidden
                && !host
                    .world()
                    .resource::<sim::SpecialState>()
                    .hidden
                    .contains(&(HUMAN, view.id))
        })
        .map(|(view, transform, _)| {
            (
                view.id,
                transform.translation.truncate() - Vec2::new(0.0, GROUND_OFFSET_PX),
            )
        })
        .collect()
}

/// The enemy building covering `cell` that the human player is shown, if any.
fn shown_enemy_building(host: &SimHost, cell: Option<CellPos>) -> Option<SimId> {
    let cell = cell?;
    if let Some(id) = host.props().and_then(|p| p.id_at(cell)).filter(|id| {
        host.world()
            .get_resource::<sim::SpecialState>()
            .is_some_and(|state| {
                state.garrisons.get(id).is_some_and(|g| g.owner != HUMAN)
                    && !state.hidden.contains(&(HUMAN, *id))
            })
            && host
                .players()
                .and_then(|p| p.get(HUMAN))
                .is_some_and(|p| p.vision().is_visible(cell))
    }) {
        return Some(id);
    }
    let id = host.grid()?.building(cell)?;
    let players = host.players()?;
    let mut buildings = host.world().try_query::<(&SimId, &Owner, &Building)>()?;
    buildings
        .iter(host.world())
        .find(|(other, ..)| **other == id)
        .filter(|(_, owner, building)| {
            owner.0 != HUMAN
                && crate::buildings::building_shown(owner.0, building.footprint, players)
        })
        .map(|(id, ..)| *id)
}

/// Ordinary scenery targets on explored ground. RA2 trees require force fire;
/// the legacy ruleset retains its ordinary tree targeting.
fn shown_scenery(host: &SimHost, cell: Option<CellPos>) -> Option<SimId> {
    let cell = cell?;
    let props = host.props()?;
    let id = props.id_at(cell)?;
    // RA9: apply shipped RA2's disabled TreeTargeting to ordinary cursor targets.
    // Ctrl force fire still finds trees independently through ra7_controls::known_at.
    if props.at(cell)?.kind == sim::PropKind::Trees
        && host
            .definitions()
            .is_some_and(|defs| defs.rules().is_some())
    {
        return None;
    }
    if host
        .world()
        .get_resource::<sim::SpecialState>()
        .is_some_and(|s| s.garrisons.contains_key(&id))
    {
        return None;
    }
    host.players()?
        .get(HUMAN)?
        .vision()
        .is_explored(cell)
        .then_some(id)
}

/// The ids of the human player's units that carry component `T`.
fn own_units_with<T: Component>(host: &SimHost) -> BTreeSet<SimId> {
    let Some(mut units) = host
        .world()
        .try_query_filtered::<(&SimId, &Owner), With<T>>()
    else {
        return BTreeSet::new();
    };
    units
        .iter(host.world())
        .filter(|(_, owner)| owner.0 == HUMAN)
        .map(|(id, _)| *id)
        .collect()
}

/// Whether a unit of `domain` may stand on terrain `cell` (buildings aside).
fn terrain_enterable(cell: &sim::Cell, domain: sim::Domain) -> bool {
    match domain {
        sim::Domain::Land => cell.passable,
        sim::Domain::Naval => cell.water,
        sim::Domain::Amphibious => cell.passable || cell.water,
        sim::Domain::Air => true,
    }
}

/// The human player's building covering `cell`, if any.
fn own_building(host: &SimHost, cell: Option<CellPos>) -> Option<SimId> {
    let cell = cell?;
    if let Some(id) = host
        .props()
        .and_then(|p| p.id_at(cell))
        .filter(|id| own_garrisons(host).contains(id))
    {
        return Some(id);
    }
    let id = host.grid()?.building(cell)?;
    let mut buildings = host.world().try_query::<(&SimId, &Owner)>()?;
    buildings
        .iter(host.world())
        .find(|(other, owner)| **other == id && owner.0 == HUMAN)
        .map(|(id, _)| *id)
}

/// Works out what a right click would order ([`order_intent`]) before it happens.
#[allow(clippy::too_many_arguments)]
pub(crate) fn preview_order(
    keys: Res<ButtonInput<KeyCode>>,
    hovered: Res<HoveredCell>,
    selection: Res<Selection>,
    building: Res<SelectedBuilding>,
    mode: Res<InterfaceMode>,
    terrain: Option<Res<MapTerrain>>,
    host: Res<SimHost>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform)>,
    views: Query<(&UnitView, &Transform, &Visibility)>,
    mut preview: ResMut<OrderPreview>,
) {
    let (camera, camera_transform) = *camera;
    let point = cursor_world(&window, camera, camera_transform);
    let over_map = window.focused
        && host.running()
        && *mode == InterfaceMode::Normal
        && window
            .cursor_position()
            .is_some_and(|p| !over_sidebar(p, window.width()));
    let mut facts = HoverFacts {
        over_map,
        selected: selection.0.len(),
        // Only a unit-producing building has a rally point: a selected Construction Yard
        // offers none.
        producer: selection.0.is_empty()
            && building
                .0
                .is_some_and(|id| own_producers(&host).contains(&id)),
        cell: hovered.0,
        ..default()
    };
    if over_map && !selection.0.is_empty() {
        let building = shown_enemy_building(&host, hovered.0);
        facts.enemy = point
            .and_then(|point| enemy_at(point, &shown_enemy_units(&host, &views), building))
            .or(building);
        let own: Vec<(SimId, Vec2)> = views
            .iter()
            .filter(|(view, ..)| view.owner == HUMAN && !launched(&host, view.id))
            .map(|(view, transform, _)| {
                (
                    view.id,
                    transform.translation.truncate() - Vec2::new(0.0, GROUND_OFFSET_PX),
                )
            })
            .collect();
        facts.friendly = point
            .and_then(|point| unit_at(point, &own))
            .filter(|id| !(selection.0.len() == 1 && selection.0.contains(id)))
            .or_else(|| own_building(&host, hovered.0));
        let armed = own_units_with::<Weapon>(&host);
        let harvesters = own_units_with::<Harvester>(&host);
        facts.armed = selection.0.iter().any(|id| armed.contains(id));
        // Scenery is a target for armed units only; others cannot do anything with it.
        if facts.enemy.is_none() && facts.armed {
            facts.enemy = shown_scenery(&host, hovered.0);
        }
        facts.harvesters = selection.0.iter().any(|id| harvesters.contains(id));
        let cell = hovered
            .0
            .zip(terrain.as_deref())
            .and_then(|(cell, terrain)| terrain.0.cell(cell));
        facts.ore = cell.is_some_and(|cell| cell.ore > 0);
        let explored = hovered.0.is_some_and(|cell| {
            host.players()
                .and_then(|players| players.get(HUMAN))
                .is_some_and(|state| state.vision().is_explored(cell))
        });
        facts.enterable = !explored
            || cell.is_some_and(|cell| {
                let grid = host.grid();
                selection.0.iter().any(|id| {
                    terrain_enterable(cell, grid.map_or(sim::Domain::Land, |g| g.domain_of(*id)))
                })
            });
    }
    let context_target = facts
        .enemy
        .or(facts.friendly)
        .or_else(|| crate::ra7_controls::known_at(&host, hovered.0));
    let context = over_map
        .then(|| {
            context_target.and_then(|target| {
                selection.0.iter().find_map(|unit| {
                    sim::special::interaction_label(host.world(), HUMAN, *unit, target)
                })
            })
        })
        .flatten();
    let base = context.map_or_else(|| order_intent(facts), OrderIntent::Interact);
    let modified = over_map
        .then(|| {
            crate::ra7_controls::modified_order(
                &selection.0,
                hovered.0,
                context_target,
                keys.any_pressed([
                    KeyCode::ControlLeft,
                    KeyCode::ControlRight,
                    KeyCode::SuperLeft,
                    KeyCode::SuperRight,
                ]),
                keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
                keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]),
                keys.pressed(KeyCode::KeyZ),
            )
        })
        .flatten();
    let intent = match modified {
        Some(Command::ForceFire { .. }) => OrderIntent::ForceFire,
        Some(Command::ForceMove { .. }) => OrderIntent::ForceMove,
        Some(Command::QueueMove { .. }) => OrderIntent::Waypoint,
        Some(Command::AttackMove { .. }) => OrderIntent::AttackMove,
        _ => base,
    };
    let next = OrderPreview {
        intent,
        target: match intent {
            OrderIntent::Interact(_) | OrderIntent::ForceFire => context_target,
            OrderIntent::Attack => facts.enemy,
            OrderIntent::Friendly => facts.friendly,
            OrderIntent::Rally => building.0,
            _ => None,
        },
        point,
    };
    if *preview != next {
        *preview = next;
    }
}

/// The label beside the cursor that names the order a right click would give.
#[derive(Component, Debug)]
pub struct IntentLabel;

fn spawn_intent_label(mut commands: Commands) {
    commands.spawn((
        IntentLabel,
        Text::new(""),
        TextFont::from_font_size(12.0),
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            display: Display::None,
            ..default()
        },
        GlobalZIndex(5),
        bevy::ui::FocusPolicy::Pass,
        Pickable::IGNORE,
    ));
}

/// Shows the preview: the operating system cursor (crosshair over an attack target, "not
/// allowed" where nothing would happen) and the coloured label beside it.
fn show_intent_cursor(
    mut commands: Commands,
    preview: Res<OrderPreview>,
    window: Single<(Entity, &Window), With<PrimaryWindow>>,
    mut label: Query<(&mut Text, &mut TextColor, &mut Node), With<IntentLabel>>,
    mut shown: Local<Option<OrderIntent>>,
) {
    let (entity, window) = *window;
    if *shown != Some(preview.intent) {
        use bevy::window::{CursorIcon, SystemCursorIcon};
        let icon = match preview.intent {
            OrderIntent::Attack | OrderIntent::ForceFire | OrderIntent::AttackMove => {
                SystemCursorIcon::Crosshair
            }
            OrderIntent::Invalid => SystemCursorIcon::NotAllowed,
            OrderIntent::Interact(_)
            | OrderIntent::Friendly
            | OrderIntent::Harvest
            | OrderIntent::Rally
            | OrderIntent::ForceMove
            | OrderIntent::Waypoint => SystemCursorIcon::Pointer,
            OrderIntent::None | OrderIntent::Move => SystemCursorIcon::Default,
        };
        commands.entity(entity).insert(CursorIcon::System(icon));
        *shown = Some(preview.intent);
    }
    let Ok((mut text, mut colour, mut node)) = label.single_mut() else {
        return;
    };
    let cursor = window.cursor_position();
    let display = if preview.intent != OrderIntent::None && cursor.is_some() {
        Display::Flex
    } else {
        Display::None
    };
    if node.display != display {
        node.display = display;
    }
    if let Some(cursor) = cursor {
        node.left = Val::Px(cursor.x + 16.0);
        node.top = Val::Px(cursor.y + 14.0);
    }
    if text.0 != preview.intent.label() {
        text.0 = preview.intent.label().into();
    }
    if colour.0 != preview.intent.colour() {
        colour.0 = preview.intent.colour();
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn issue_orders(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    hovered: Res<HoveredCell>,
    selection: Res<Selection>,
    mode: Res<InterfaceMode>,
    terrain: Option<Res<MapTerrain>>,
    host: Res<SimHost>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform)>,
    views: Query<(&UnitView, &Transform, &Visibility)>,
    mut sink: ResMut<CommandSink>,
    mut feedback: ResMut<OrderFeedback>,
    preview: Res<OrderPreview>,
    time: Res<Time>,
) {
    if !window.focused {
        return;
    }
    let over_map = window
        .cursor_position()
        .is_some_and(|p| !over_sidebar(p, window.width()));
    let mut commands = Vec::new();
    if over_map
        && *mode == InterfaceMode::Normal
        && (buttons.just_pressed(MouseButton::Left) || buttons.just_pressed(MouseButton::Right))
    {
        let modified = crate::ra7_controls::modified_order(
            &selection.0,
            hovered.0,
            preview
                .target
                .or_else(|| crate::ra7_controls::known_at(&host, hovered.0)),
            keys.any_pressed([
                KeyCode::ControlLeft,
                KeyCode::ControlRight,
                KeyCode::SuperLeft,
                KeyCode::SuperRight,
            ]),
            keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
            keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]),
            keys.pressed(KeyCode::KeyZ),
        );
        if let Some(command) = modified {
            sink.issue([command]);
            return;
        }
        if buttons.just_pressed(MouseButton::Right)
            && let OrderIntent::Interact(_) = preview.intent
            && let Some(target) = preview.target
        {
            sink.issue(
                selection
                    .0
                    .iter()
                    .filter(|unit| {
                        sim::special::interaction_label(host.world(), HUMAN, **unit, target)
                            .is_some()
                    })
                    .map(|unit| Command::Interact {
                        unit: *unit,
                        target,
                    }),
            );
            return;
        }
    }
    // In a mode of `crate::construction`, a right click leaves the mode instead.
    // A right click where nothing could happen (the preview shows "CANNOT GO") orders nothing.
    if buttons.just_pressed(MouseButton::Right)
        && over_map
        && *mode == InterfaceMode::Normal
        && preview.intent == OrderIntent::Rally
    {
        // A factory is selected: the cell becomes its rally point.
        commands.extend(rally_command(preview.target, hovered.0));
    } else if buttons.just_pressed(MouseButton::Right)
        && over_map
        && *mode == InterfaceMode::Normal
        && preview.intent != OrderIntent::Invalid
    {
        let (camera, camera_transform) = *camera;
        let building = shown_enemy_building(&host, hovered.0);
        let enemy = cursor_world(&window, camera, camera_transform)
            .and_then(|point| enemy_at(point, &shown_enemy_units(&host, &views), building))
            .or(building)
            .or_else(|| {
                let armed = own_units_with::<Weapon>(&host);
                selection
                    .0
                    .iter()
                    .any(|id| armed.contains(id))
                    .then(|| shown_scenery(&host, hovered.0))?
            });
        match enemy {
            Some(target) => {
                let armed = own_units_with::<Weapon>(&host);
                commands.extend(attack_commands(&selection.0, &armed, target, hovered.0));
            }
            None => {
                // Ore as the map shows it: the simulation's current ore of a cell the player does
                // not see is not revealed by the choice between harvesting and moving.
                let ore = hovered
                    .0
                    .zip(terrain.as_deref())
                    .and_then(|(cell, terrain)| terrain.0.cell(cell))
                    .is_some_and(|cell| cell.ore > 0);
                let harvesters = own_units_with::<Harvester>(&host);
                commands.extend(right_click_commands(
                    &selection.0,
                    &harvesters,
                    hovered.0,
                    ore,
                ));
            }
        }
    }
    for action in pressed_hotkeys(&keys) {
        match action {
            Hotkey::Stop => commands.extend(stop_command(&selection.0)),
            Hotkey::AttackMove if over_map => {
                let ore = hovered
                    .0
                    .zip(terrain.as_deref())
                    .and_then(|(c, t)| t.0.cell(c))
                    .is_some_and(|c| c.ore > 0);
                let miners = own_units_with::<Harvester>(&host);
                if ore {
                    let workers = selection.0.intersection(&miners).copied().collect();
                    let fighters = selection.0.difference(&miners).copied().collect();
                    commands.extend(right_click_commands(&workers, &miners, hovered.0, true));
                    commands.extend(attack_move_command(&fighters, hovered.0));
                } else {
                    commands.extend(attack_move_command(&selection.0, hovered.0));
                }
            }
            _ => {}
        }
    }
    if sink.issue(commands) > 0 && over_map {
        feedback.cell = hovered.0;
        feedback.until = time.elapsed_secs_f64() + 0.65;
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_selection(
    order: Res<OrderPreview>,
    host: Res<SimHost>,
    feedback: Res<OrderFeedback>,
    time: Res<Time>,
    selection: Res<Selection>,
    building: Res<SelectedBuilding>,
    drag: Res<DragBox>,
    map: Option<Res<MapView>>,
    camera: Single<(&Camera, &GlobalTransform)>,
    views: Query<(&UnitView, &Transform)>,
    visible_views: Query<(&UnitView, &Visibility)>,
    mut gizmos: Gizmos,
) {
    let Some(map) = map else {
        return;
    };
    if let Some(id) = building.0 {
        draw_selected_building(id, &host, &map, time.elapsed_secs(), &mut gizmos);
    }
    // A small diamond under each selected unit, a quarter of a cell from its ground point.
    let marker = cell_corners(CellPos::new(0, 0), map.tile).map(|corner| {
        (corner - crate::projection::cell_centre(CellPos::new(0, 0), map.tile)) * 0.6
    });
    let green = Color::srgb(0.3, 1.0, 0.3);
    let (camera, camera_transform) = *camera;
    let preview = drag
        .start
        .zip(drag.last)
        .filter(|(a, b)| a.distance(*b) >= DRAG_THRESHOLD_PX)
        .and_then(|(a, b)| {
            Some(Rect::from_corners(
                camera.viewport_to_world_2d(camera_transform, a).ok()?,
                camera.viewport_to_world_2d(camera_transform, b).ok()?,
            ))
        });
    let shown_owned: BTreeSet<_> = visible_views
        .iter()
        .filter(|(view, visibility)| {
            **visibility != Visibility::Hidden && owned_commandable_unit(&host, view)
        })
        .map(|(view, _)| view.id)
        .collect();
    for (view, transform) in &views {
        if !shown_owned.contains(&view.id) {
            continue;
        }
        if !selection.0.contains(&view.id)
            && !(view.owner == HUMAN
                && preview.is_some_and(|r| r.contains(transform.translation.truncate())))
        {
            continue;
        }
        let ground = transform.translation.truncate() - Vec2::new(0.0, GROUND_OFFSET_PX);
        let [top, right, bottom, left] = marker.map(|corner| ground + corner);
        gizmos.linestrip_2d([top, right, bottom, left, top], green);
        if view.altitude > 1.0 {
            // An aircraft: the marker again on the ground straight below it, where its shadow
            // lies, and a plumb line between the two that shows how high it flies.
            let below = ground - Vec2::Y * view.altitude;
            let [top, right, bottom, left] = marker.map(|corner| below + corner * 0.7);
            let faint = green.with_alpha(0.55);
            gizmos.linestrip_2d([top, right, bottom, left, top], faint);
            gizmos.line_2d(below, ground, faint);
        }
    }
    if time.elapsed_secs_f64() < feedback.until
        && let Some(cell) = feedback.cell
    {
        let centre = crate::projection::cell_centre(cell, map.tile);
        let r = 8.0 + (feedback.until - time.elapsed_secs_f64()) as f32 * 14.0;
        gizmos.linestrip_2d(
            [
                centre + Vec2::Y * r,
                centre + Vec2::X * r * 2.0,
                centre - Vec2::Y * r,
                centre - Vec2::X * r * 2.0,
                centre + Vec2::Y * r,
            ],
            Color::srgb(0.9, 0.85, 0.4),
        );
    }
    draw_order_preview(&order, &host, &map, &views, &mut gizmos);
    // The box while the left button is dragged.

    if let (Some(start), Some(end)) = (
        drag.start,
        drag.last
            .and_then(|p| camera.viewport_to_world_2d(camera_transform, p).ok()),
    ) && let Ok(start) = camera.viewport_to_world_2d(camera_transform, start)
    {
        let rect = Rect::from_corners(start, end);
        gizmos.linestrip_2d(
            [
                rect.min,
                Vec2::new(rect.max.x, rect.min.y),
                rect.max,
                Vec2::new(rect.min.x, rect.max.y),
                rect.min,
            ],
            green,
        );
    }
}

/// The corners of a footprint on the ground, clockwise from its top corner on screen.
fn footprint_outline(footprint: sim::Footprint, map: &MapView) -> [Vec2; 4] {
    let corner = |x: i32, y: i32| crate::projection::to_world(x as f32, y as f32, map.tile);
    let (x0, y0) = (footprint.origin.x, footprint.origin.y);
    let (x1, y1) = (
        x0 + i32::from(footprint.width),
        y0 + i32::from(footprint.height),
    );
    [
        corner(x0, y0),
        corner(x1, y0),
        corner(x1, y1),
        corner(x0, y1),
    ]
}

/// A rally flag standing on `ground`: a pole with a pennant that waves with `phase` (seconds),
/// and a small diamond at its foot.
fn draw_rally_flag(gizmos: &mut Gizmos, ground: Vec2, colour: Color, phase: f32) {
    let top = ground + Vec2::Y * 26.0;
    let wave = (phase * 5.0).sin() * 1.5;
    gizmos.line_2d(ground, top, colour);
    gizmos.linestrip_2d(
        [
            top,
            top + Vec2::new(13.0, -4.0 + wave),
            top + Vec2::new(0.0, -9.0),
        ],
        colour,
    );
    gizmos.line_2d(
        top + Vec2::new(0.0, -4.5),
        top + Vec2::new(13.0, -4.0 + wave),
        colour,
    );
    let (w, h) = (8.0, 4.0);
    gizmos.linestrip_2d(
        [
            ground + Vec2::Y * h,
            ground + Vec2::X * w,
            ground - Vec2::Y * h,
            ground - Vec2::X * w,
            ground + Vec2::Y * h,
        ],
        colour,
    );
}

/// The selected unit-producing building: a bracket around its footprint, and its rally point as
/// a flag with a dashed line from the building, if it has one.
fn draw_selected_building(
    id: SimId,
    host: &SimHost,
    map: &MapView,
    phase: f32,
    gizmos: &mut Gizmos,
) {
    if let Some(g) = host
        .world()
        .get_resource::<sim::SpecialState>()
        .and_then(|s| s.garrisons.get(&id))
    {
        let outline = footprint_outline(
            sim::Footprint {
                origin: g.cell,
                width: 1,
                height: 1,
            },
            map,
        );
        for i in 0..4 {
            gizmos.line_2d(outline[i], outline[(i + 1) % 4], Color::srgb(0.3, 1.0, 0.3));
        }
        return;
    }
    let Some(mut buildings) = host
        .world()
        .try_query::<(&SimId, &Building, Option<&Producer>)>()
    else {
        return;
    };
    let Some((_, building, producer)) = buildings
        .iter(host.world())
        .find(|(other, ..)| **other == id)
    else {
        return;
    };
    let outline = footprint_outline(building.footprint, map);
    let centre = outline.iter().copied().sum::<Vec2>() / 4.0;
    let green = Color::srgb(0.3, 1.0, 0.3);
    // Corner brackets, as around a selected unit, a little outside the walls.
    for i in 0..4 {
        let corner = outline[i] + (outline[i] - centre).normalize_or_zero() * 5.0;
        for neighbour in [outline[(i + 1) % 4], outline[(i + 3) % 4]] {
            let along = (neighbour - outline[i]).normalize_or_zero();
            gizmos.line_2d(corner, corner + along * 14.0, green);
        }
    }
    let Some(rally) = producer.and_then(|producer| producer.rally) else {
        return;
    };
    let flag = crate::projection::cell_centre(rally, map.tile);
    // A dashed line that runs from the building to the flag.
    let length = centre.distance(flag);
    let direction = (flag - centre).normalize_or_zero();
    let (dash, gap) = (7.0, 5.0);
    let mut at = (phase * 24.0) % (dash + gap);
    while at < length {
        let from = at.max(0.0);
        let to = (at + dash).min(length);
        gizmos.line_2d(
            centre + direction * from,
            centre + direction * to,
            RALLY_COLOUR.with_alpha(0.7),
        );
        at += dash + gap;
    }
    draw_rally_flag(gizmos, flag, RALLY_COLOUR, phase);
}

/// The marker of the order a right click would give: a red reticle on the cursor and a red
/// bracket around the target of an attack, a ring around a friendly target, a cross where
/// nothing could happen, and a small diamond for a move or harvest.
fn draw_order_preview(
    preview: &OrderPreview,
    host: &SimHost,
    map: &MapView,
    views: &Query<(&UnitView, &Transform)>,
    gizmos: &mut Gizmos,
) {
    let (Some(point), colour) = (preview.point, preview.intent.colour()) else {
        return;
    };
    let target_outline = |gizmos: &mut Gizmos, id: SimId, colour: Color| {
        if let Some((_, transform)) = views.iter().find(|(view, _)| view.id == id) {
            let body = transform.translation.truncate();
            gizmos.ellipse_2d(
                Isometry2d::from_translation(body),
                Vec2::new(26.0, 20.0),
                colour,
            );
            gizmos.ellipse_2d(
                Isometry2d::from_translation(body),
                Vec2::new(30.0, 24.0),
                colour,
            );
            return;
        }
        let Some(mut buildings) = host.world().try_query::<(&SimId, &Building)>() else {
            return;
        };
        if let Some((_, building)) = buildings
            .iter(host.world())
            .find(|(other, _)| **other == id)
        {
            let f = building.footprint;
            let corner = |x: i32, y: i32| crate::projection::to_world(x as f32, y as f32, map.tile);
            let (x0, y0) = (f.origin.x, f.origin.y);
            let (x1, y1) = (x0 + f.width as i32, y0 + f.height as i32);
            let outline = [
                corner(x0, y0),
                corner(x1, y0),
                corner(x1, y1),
                corner(x0, y1),
            ];
            for grow in [0.0, 4.0] {
                let centre = outline.iter().copied().sum::<Vec2>() / 4.0;
                let ring = outline.map(|p| p + (p - centre).normalize_or_zero() * grow);
                gizmos.linestrip_2d([ring[0], ring[1], ring[2], ring[3], ring[0]], colour);
            }
        }
    };
    match preview.intent {
        OrderIntent::None => {}
        OrderIntent::Attack | OrderIntent::ForceFire | OrderIntent::AttackMove => {
            for radius in [9.0, 14.0] {
                gizmos.circle_2d(Isometry2d::from_translation(point), radius, colour);
            }
            for direction in [Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y] {
                gizmos.line_2d(point + direction * 5.0, point + direction * 20.0, colour);
            }
            if let Some(target) = preview.target {
                target_outline(gizmos, target, colour);
            }
        }
        OrderIntent::Friendly | OrderIntent::Interact(_) => {
            if let Some(target) = preview.target {
                target_outline(gizmos, target, colour);
            }
        }
        OrderIntent::Invalid => {
            let d = Vec2::splat(8.0);
            gizmos.line_2d(point - d, point + d, colour);
            gizmos.line_2d(
                point + Vec2::new(-d.x, d.y),
                point + Vec2::new(d.x, -d.y),
                colour,
            );
            gizmos.circle_2d(Isometry2d::from_translation(point), 12.0, colour);
        }
        OrderIntent::Rally => {
            // Where the flag would stand.
            draw_rally_flag(gizmos, point, colour.with_alpha(0.6), 0.0);
        }
        OrderIntent::Move
        | OrderIntent::Harvest
        | OrderIntent::ForceMove
        | OrderIntent::Waypoint => {
            let r = 6.0;
            gizmos.linestrip_2d(
                [
                    point + Vec2::Y * r,
                    point + Vec2::X * r * 2.0,
                    point - Vec2::Y * r,
                    point - Vec2::X * r * 2.0,
                    point + Vec2::Y * r,
                ],
                colour,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    include!("selection_pad_native.rs");
    include!("privacy_selection_tests.rs");
    include!("privacy_order_tests.rs");
    use super::*;

    fn input_app() -> (App, Entity, SimId) {
        input_app_on(crate::sim_host::tests::demo_host())
    }

    fn input_app_on(host: SimHost) -> (App, Entity, SimId) {
        let mut app = App::new();
        let mut q = host
            .world()
            .try_query::<(Entity, &SimId, &Owner, &sim::Position)>()
            .unwrap();
        let (unit, id, _, pos) = q
            .iter(host.world())
            .find(|(_, _, o, _)| o.0 == HUMAN)
            .unwrap();
        let (unit, id, pos) = (unit, *id, pos.0);
        app.insert_resource(host)
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Time>()
            .init_resource::<Selection>()
            .init_resource::<SelectedBuilding>()
            .init_resource::<DragBox>()
            .init_resource::<InterfaceMode>()
            .init_resource::<CommandSink>()
            .init_resource::<OrderFeedback>()
            .init_resource::<OrderPreview>()
            .init_resource::<HoveredCell>()
            .add_systems(Update, (select_units, issue_orders).chain());
        let window = app
            .world_mut()
            .spawn((
                Window {
                    resolution: (1280, 800).into(),
                    focused: true,
                    ..default()
                },
                PrimaryWindow,
            ))
            .id();
        app.world_mut().spawn((
            Camera {
                computed: bevy::camera::ComputedCameraValues {
                    clip_from_view: Mat4::orthographic_rh(
                        -640.0, 640.0, -400.0, 400.0, 0.0, 1000.0,
                    ),
                    target_info: Some(bevy::camera::RenderTargetInfo {
                        physical_size: UVec2::new(1280, 800),
                        scale_factor: 1.0,
                    }),
                    ..default()
                },
                ..default()
            },
            GlobalTransform::default(),
        ));
        app.world_mut().spawn((
            UnitView {
                unit,
                id,
                owner: HUMAN,
                previous: pos,
                current: pos,
                altitude: 0.0,
                drawn: None,
                stride: 0.0,
            },
            Transform::from_xyz(0.0, 0.0, 10.0),
            Visibility::Visible,
        ));
        (app, window, id)
    }
    fn cursor(app: &mut App, window: Entity, at: Option<Vec2>) {
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .set_cursor_position(at);
    }
    fn click(app: &mut App, button: MouseButton) {
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(button);
        app.update();
        let mut buttons = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        buttons.clear();
        buttons.release(button);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
    }

    // RA9 scenarios use shipped definitions for shipped targeting, and the independent
    // legacy fixture only for the rules-less compatibility control. Every Prop is installed
    // by the same world-object installer as an actual client/headless match.
    fn ra9_prop_scene(
        feature: u8,
        legacy: bool,
        explored: bool,
        actor: &str,
    ) -> (SimHost, SimId, SimId) {
        let cell = CellPos::new(11, 16);
        let mut selected = None;
        let host = crate::sim_host::tests::fixture_host(|world| {
            let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let data = if legacy {
                root.join("../sim/tests/fixtures/data/legacy")
            } else {
                root.join("../../assets/data")
            };
            let defs = if legacy {
                sim::load_match_definitions(
                    data.join("units.ron"),
                    data.join("economy.ron"),
                    data.join("combat.ron"),
                )
                .expect("legacy combat definitions")
                .with_arsenal(data.join("arsenal.ron"))
                .expect("legacy metadata")
            } else {
                sim::load_ruleset(&data).expect("shipped ruleset")
            };
            let mut skirmish =
                sim::load_skirmish(data.join("skirmish.ron"), &defs).expect("skirmish definitions");
            let ai = sim::load_ai(data.join("ai.ron"), &defs).expect("AI definitions");
            // Tiny input scenario: no starting armies or AI controller can fight the Prop.
            // Both real starting yards survive, so the client has a running match.
            for faction in &mut skirmish.factions {
                faction.units.clear();
                if faction.side.is_none() {
                    faction.buildings.truncate(1);
                }
            }
            skirmish.maps[0].starts = vec![CellPos::new(2, 2), CellPos::new(26, 26)];
            world.insert_resource(defs);
            let open = sim::Cell {
                passable: true,
                water: false,
                buildable: true,
                ore: 0,
            };
            let mut grid = sim::Grid::from_cells(32, 32, vec![open; 32 * 32]).expect("grid");
            grid.set_terrain(
                cell,
                sim::Cell {
                    passable: feature == 8,
                    buildable: false,
                    ..open
                },
            );
            let mut looks = vec![0; 32 * 32];
            looks[16 * 32 + 11] = feature << 4;
            grid.mark_world_features(&looks);
            world.insert_resource(grid);
            sim::setup_match_configured(
                world,
                &skirmish,
                &ai,
                0,
                &[
                    sim::PlayerSetup {
                        player: HUMAN,
                        faction: 0,
                        start: 0,
                        controller: sim::Controller::Human,
                    },
                    sim::PlayerSetup {
                        player: sim::PlayerId(1),
                        faction: 1,
                        start: 1,
                        controller: sim::Controller::Human,
                    },
                ],
                &sim::MatchOptions {
                    unit_count: Some(0),
                    mcv_start: false,
                    free_for_all: true,
                    crates: false,
                    ore_grows: false,
                    ..default()
                },
            )
            .expect("two human-controlled surviving yards and real world objects");
            selected =
                Some(sim::spawn_unit(world, HUMAN, actor, CellPos::new(10, 16)).expect("actor"));
            if explored {
                world.run_schedule(sim::SimSchedule);
            }
        });
        assert!(host.running(), "preview_order requires a live client match");
        let actor = selected.expect("selected actor");
        let prop = host.props().expect("props").id_at(cell).expect("prop");
        assert_eq!(
            host.definitions().expect("definitions").rules().is_none(),
            legacy
        );
        assert_eq!(
            host.players()
                .expect("players")
                .get(HUMAN)
                .expect("human")
                .vision()
                .is_explored(cell),
            explored
        );
        assert!(host.world().iter_entities().any(|e| {
            e.get::<SimId>() == Some(&actor)
                && e.get::<sim::Health>().is_some_and(|h| h.hit_points > 0)
        }));
        (host, actor, prop)
    }

    fn ra9_prop_input(host: SimHost, actor: SimId) -> (App, Entity) {
        let terrain = host.grid().expect("grid").clone();
        let (mut app, window, selected) = input_app_on(host);
        assert_eq!(selected, actor, "the only own actor supplies the view");
        app.insert_resource(MapTerrain(terrain)).add_systems(
            Update,
            preview_order.before(issue_orders).after(select_units),
        );
        app.world_mut().resource_mut::<Selection>().0.insert(actor);
        app.world_mut().resource_mut::<HoveredCell>().0 = Some(CellPos::new(11, 16));
        cursor(&mut app, window, Some(Vec2::new(800.0, 400.0)));
        (app, window)
    }

    fn ra9_right_click_commands(app: &mut App) -> Vec<Command> {
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
        crate::sim_host::tests::take_commands(app)
    }

    fn ra9_run_input(app: &mut App, commands: Vec<Command>) -> Vec<sim::SimEvent> {
        crate::sim_host::tests::run(&mut app.world_mut().resource_mut::<SimHost>(), commands)
    }

    #[test]
    fn ra9_ra2_tree_ordinary_preview_and_right_click_require_force_fire() {
        let (host, actor, prop) = ra9_prop_scene(4, false, true, "grizzly");
        assert!(own_units_with::<Weapon>(&host).contains(&actor));
        assert_eq!(
            host.props().unwrap().get(prop).unwrap().1.kind,
            sim::PropKind::Trees
        );
        assert_eq!(shown_scenery(&host, Some(CellPos::new(11, 16))), None);
        assert_eq!(
            crate::ra7_controls::known_at(&host, Some(CellPos::new(11, 16))),
            Some(prop)
        );
        let hp = host.props().unwrap().get(prop).unwrap().1.hit_points;
        let (mut app, _) = ra9_prop_input(host, actor);
        app.update();
        let preview = *app.world().resource::<OrderPreview>();
        assert_eq!(
            preview.intent,
            OrderIntent::Invalid,
            "the visible tree blocks ground"
        );
        assert_eq!(preview.target, None);
        assert_eq!(preview.intent.label(), "CANNOT GO");
        assert_eq!(ra9_right_click_commands(&mut app), []);
        assert_eq!(app.world().resource::<CommandSink>().accepted(), 0);
        let events = ra9_run_input(&mut app, vec![]);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, sim::SimEvent::WeaponFired { .. }))
        );
        assert_eq!(
            app.world()
                .resource::<SimHost>()
                .props()
                .unwrap()
                .get(prop)
                .unwrap()
                .1
                .hit_points,
            hp
        );
    }

    #[test]
    fn ra9_non_tree_scenery_and_legacy_trees_keep_ordinary_attack_commands() {
        for (feature, legacy, kind) in [
            (5, false, sim::PropKind::House),
            (6, false, sim::PropKind::CityBlock),
            (7, false, sim::PropKind::Tower),
            (8, false, sim::PropKind::Bridge),
            (4, true, sim::PropKind::Trees),
        ] {
            let actor_type = if legacy { "light_tank" } else { "grizzly" };
            let (host, actor, prop) = ra9_prop_scene(feature, legacy, true, actor_type);
            assert!(own_units_with::<Weapon>(&host).contains(&actor));
            assert_eq!(host.props().unwrap().get(prop).unwrap().1.kind, kind);
            assert_eq!(shown_scenery(&host, Some(CellPos::new(11, 16))), Some(prop));
            let (mut app, _) = ra9_prop_input(host, actor);
            app.update();
            assert_eq!(
                app.world().resource::<OrderPreview>().intent,
                OrderIntent::Attack,
                "{kind:?}"
            );
            assert_eq!(app.world().resource::<OrderPreview>().target, Some(prop));
            let commands = ra9_right_click_commands(&mut app);
            assert_eq!(
                commands,
                [Command::Attack {
                    units: vec![actor],
                    target: prop
                }]
            );
            let events = ra9_run_input(&mut app, commands);
            assert!(
                !events
                    .iter()
                    .any(|e| matches!(e, sim::SimEvent::CommandRejected { .. })),
                "{events:?}"
            );
            assert!(events.iter().any(|e| matches!(e,
                sim::SimEvent::WeaponFired { attacker, target, .. } if *attacker == actor && *target == prop
            )), "{kind:?}: {events:?}");
        }
    }

    #[test]
    fn ra9_ctrl_tree_click_keeps_entity_force_fire_and_native_damage() {
        let (host, actor, prop) = ra9_prop_scene(4, false, true, "grizzly");
        let hp = host.props().unwrap().get(prop).unwrap().1.hit_points;
        let (mut app, _) = ra9_prop_input(host, actor);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ControlLeft);
        app.update();
        assert_eq!(
            app.world().resource::<OrderPreview>().intent,
            OrderIntent::ForceFire
        );
        assert_eq!(app.world().resource::<OrderPreview>().target, Some(prop));
        let commands = ra9_right_click_commands(&mut app);
        assert_eq!(
            commands,
            [Command::ForceFire {
                units: vec![actor],
                target: sim::ForceTarget::Entity(prop),
            }]
        );
        let events = ra9_run_input(&mut app, commands);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, sim::SimEvent::CommandRejected { .. })),
            "{events:?}"
        );
        assert!(events.iter().any(|e| matches!(e,
            sim::SimEvent::WeaponFired { attacker, target, .. } if *attacker == actor && *target == prop
        )), "{events:?}");
        // Shipped Grizzly projectiles take time to arrive; observe the real impact.
        for _ in 0..16 {
            ra9_run_input(&mut app, vec![]);
        }
        assert!(
            app.world()
                .resource::<SimHost>()
                .props()
                .unwrap()
                .get(prop)
                .is_none_or(|(_, tree)| tree.hit_points < hp)
        );
    }

    #[test]
    fn ra9_unexplored_tree_and_empty_cell_never_supply_an_entity_to_cursor_or_ctrl() {
        for explored in [false, true] {
            let (host, actor, _) = ra9_prop_scene(4, false, explored, "grizzly");
            let cell = if explored {
                CellPos::new(12, 16)
            } else {
                CellPos::new(11, 16)
            };
            assert_eq!(shown_scenery(&host, Some(cell)), None);
            assert_eq!(crate::ra7_controls::known_at(&host, Some(cell)), None);
            assert_eq!(shown_scenery(&host, None), None);
            assert_eq!(crate::ra7_controls::known_at(&host, None), None);
            let (mut app, _) = ra9_prop_input(host, actor);
            app.world_mut().resource_mut::<HoveredCell>().0 = Some(cell);
            app.update();
            assert_eq!(
                app.world().resource::<OrderPreview>().intent,
                OrderIntent::Move
            );
            assert_eq!(app.world().resource::<OrderPreview>().target, None);
            assert_eq!(
                ra9_right_click_commands(&mut app),
                [Command::Move {
                    units: vec![actor],
                    target: cell
                }]
            );
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::ControlLeft);
            app.update();
            assert_eq!(
                app.world().resource::<OrderPreview>().intent,
                OrderIntent::ForceFire
            );
            assert_eq!(app.world().resource::<OrderPreview>().target, None);
            assert_eq!(
                ra9_right_click_commands(&mut app),
                [Command::ForceFire {
                    units: vec![actor],
                    target: sim::ForceTarget::Cell(cell),
                }]
            );
        }
    }

    #[test]
    fn ra9_tree_removed_by_native_damage_and_cleanup_is_missing_to_cursor_and_commands() {
        let (mut host, actor, prop) = ra9_prop_scene(4, false, true, "grizzly");
        let mut events = crate::sim_host::tests::run(
            &mut host,
            vec![Command::ForceFire {
                units: vec![actor],
                target: sim::ForceTarget::Entity(prop),
            }],
        );
        let mut destroyed = events.iter().any(|e| {
            matches!(e,
                sim::SimEvent::PropDestroyed { prop: id, .. } if *id == prop
            )
        });
        for _ in 0..256 {
            if destroyed {
                break;
            }
            events = crate::sim_host::tests::run(&mut host, vec![]);
            destroyed = events.iter().any(|e| {
                matches!(e,
                    sim::SimEvent::PropDestroyed { prop: id, .. } if *id == prop
                )
            });
        }
        assert!(destroyed, "native weapon damage must reach Cleanup");
        let cell = CellPos::new(11, 16);
        assert!(host.props().unwrap().get(prop).is_none());
        assert!(host.grid().unwrap().cell(cell).unwrap().passable);
        assert_eq!(shown_scenery(&host, Some(cell)), None);
        assert_eq!(crate::ra7_controls::known_at(&host, Some(cell)), None);
        let missing = SimId(u32::MAX - 1);
        let commands: Vec<_> = [prop, missing]
            .into_iter()
            .flat_map(|target| {
                [
                    Command::Attack {
                        units: vec![actor],
                        target,
                    },
                    Command::ForceFire {
                        units: vec![actor],
                        target: sim::ForceTarget::Entity(target),
                    },
                ]
            })
            .collect();
        let events = crate::sim_host::tests::run(&mut host, commands);
        let rejected: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                sim::SimEvent::CommandRejected { reason, .. } => Some(reason.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(
            rejected,
            [
                sim::RejectReason::UnknownTarget(prop),
                sim::RejectReason::UnknownTarget(prop),
                sim::RejectReason::UnknownTarget(missing),
                sim::RejectReason::UnknownTarget(missing),
            ]
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, sim::SimEvent::WeaponFired { .. }))
        );
        // The input app is created after completed Cleanup, using the resulting open terrain.
        let (mut app, _) = ra9_prop_input(host, actor);
        app.update();
        assert_eq!(
            app.world().resource::<OrderPreview>().intent,
            OrderIntent::Move
        );
        assert_eq!(app.world().resource::<OrderPreview>().target, None);
        assert_eq!(
            ra9_right_click_commands(&mut app),
            [Command::Move {
                units: vec![actor],
                target: cell
            }]
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ControlLeft);
        app.update();
        assert_eq!(app.world().resource::<OrderPreview>().target, None);
        assert_eq!(
            ra9_right_click_commands(&mut app),
            [Command::ForceFire {
                units: vec![actor],
                target: sim::ForceTarget::Cell(cell),
            }]
        );
    }

    #[test]
    fn ra9_tree_filter_preserves_valid_garrison_context_and_occupied_exclusion() {
        let (host, gi, house) = ra9_prop_scene(5, false, true, "gi");
        let cell = Some(CellPos::new(11, 16));
        assert_eq!(crate::ra7_controls::known_at(&host, cell), Some(house));
        assert_eq!(
            sim::special::interaction_label(host.world(), HUMAN, gi, house),
            Some("GARRISON")
        );
        let (mut app, _) = ra9_prop_input(host, gi);
        app.update();
        assert_eq!(
            app.world().resource::<OrderPreview>().intent,
            OrderIntent::Interact("GARRISON")
        );
        assert_eq!(app.world().resource::<OrderPreview>().target, Some(house));
        let commands = ra9_right_click_commands(&mut app);
        assert_eq!(
            commands,
            [Command::Interact {
                unit: gi,
                target: house
            }]
        );
        let events = ra9_run_input(&mut app, commands);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, sim::SimEvent::CommandRejected { .. })),
            "{events:?}"
        );
        let host = app.world().resource::<SimHost>();
        assert!(
            host.world()
                .resource::<sim::SpecialState>()
                .garrisons
                .contains_key(&house)
        );
        assert_eq!(shown_scenery(host, cell), None);
        assert_eq!(own_building(host, cell), Some(house));
    }

    #[test]
    fn ra9_tree_is_no_specialist_interaction_target() {
        for actor_type in ["engineer", "tanya"] {
            let (host, actor, tree) = ra9_prop_scene(4, false, true, actor_type);
            assert_eq!(
                crate::ra7_controls::known_at(&host, Some(CellPos::new(11, 16))),
                Some(tree)
            );
            assert_eq!(
                sim::special::interaction_label(host.world(), HUMAN, actor, tree),
                None
            );
            let (mut app, _) = ra9_prop_input(host, actor);
            app.update();
            assert_eq!(
                app.world().resource::<OrderPreview>().intent,
                OrderIntent::Invalid
            );
            assert_eq!(app.world().resource::<OrderPreview>().target, None);
            assert_eq!(ra9_right_click_commands(&mut app), []);
        }
    }

    #[test]
    fn ra7_modifier_cursor_names_the_command_with_the_same_precedence() {
        let (mut app, window, id) = input_app();
        app.add_systems(
            Update,
            preview_order.before(issue_orders).after(select_units),
        );
        app.world_mut().resource_mut::<Selection>().0.insert(id);
        app.world_mut().resource_mut::<HoveredCell>().0 = Some(CellPos::new(20, 20));
        cursor(&mut app, window, Some(Vec2::new(800.0, 400.0)));
        for (keys, intent, label) in [
            (
                vec![
                    KeyCode::ControlLeft,
                    KeyCode::ShiftLeft,
                    KeyCode::AltLeft,
                    KeyCode::KeyZ,
                ],
                OrderIntent::AttackMove,
                "ATTACK MOVE",
            ),
            (
                vec![KeyCode::SuperLeft, KeyCode::AltLeft],
                OrderIntent::ForceFire,
                "FORCE FIRE",
            ),
            (
                vec![KeyCode::AltLeft, KeyCode::KeyZ],
                OrderIntent::ForceMove,
                "FORCE MOVE",
            ),
            (vec![KeyCode::KeyZ], OrderIntent::Waypoint, "WAYPOINT"),
        ] {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .reset_all();
            for key in keys {
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .press(key);
            }
            app.update();
            let preview = app.world().resource::<OrderPreview>();
            assert_eq!(preview.intent, intent);
            assert_eq!(preview.intent.label(), label);
        }
    }

    #[test]
    fn ra7_visible_disguise_is_not_an_enemy_cursor_target() {
        use bevy::ecs::system::RunSystemOnce;
        let hidden_id = SimId(91_234);
        let host = crate::sim_host::tests::demo_host_with(|world| {
            world
                .resource_mut::<sim::SpecialState>()
                .hidden
                .insert((HUMAN, hidden_id));
        });
        let (mut app, _, _) = input_app_on(host);
        app.world_mut().spawn((
            UnitView {
                unit: Entity::PLACEHOLDER,
                id: hidden_id,
                owner: sim::PlayerId(1),
                previous: sim::SubPos::new(0, 0),
                current: sim::SubPos::new(0, 0),
                altitude: 0.0,
                drawn: None,
                stride: 0.0,
            },
            Transform::default(),
            // The disguise intentionally remains rendered with friendly paint.
            Visibility::Inherited,
        ));
        let targets = app
            .world_mut()
            .run_system_once(
                |host: Res<SimHost>, views: Query<(&UnitView, &Transform, &Visibility)>| {
                    shown_enemy_units(&host, &views)
                },
            )
            .unwrap();
        assert!(!targets.iter().any(|(id, _)| *id == hidden_id));
    }

    #[test]
    fn ra5_own_garrison_is_selectable_and_enemy_garrison_requires_current_visibility() {
        for (owner, hidden) in [
            (HUMAN, false),
            (sim::PlayerId(1), false),
            (sim::PlayerId(1), true),
        ] {
            let mut house = None;
            let host = crate::sim_host::tests::demo_host_with(|world| {
                let cell = CellPos::new(15, 15);
                let mut grid = world.resource_mut::<sim::Grid>();
                grid.set_terrain(
                    cell,
                    sim::Cell {
                        passable: false,
                        water: false,
                        buildable: false,
                        ore: 0,
                    },
                );
                let mut looks = vec![0; (grid.width() * grid.height()) as usize];
                looks[(cell.y as u32 * grid.width() + cell.x as u32) as usize] = 5 << 4;
                let defs = world.resource::<sim::Definitions>().clone();
                let props = sim::Props::from_looks(
                    world.resource::<sim::Grid>(),
                    &looks,
                    defs.armour_class("wood").expect("wood"),
                );
                let id = props.id_at(cell).expect("house");
                house = Some(id);
                world.insert_resource(props);
                let gi =
                    sim::spawn_unit(world, owner, "gi", CellPos::new(14, 15)).expect("occupier");
                sim::spawn_unit(world, HUMAN, "engineer", CellPos::new(15, 14)).expect("scout");
                world.run_schedule(sim::SimSchedule);
                world
                    .resource_mut::<sim::CommandQueue>()
                    .push(sim::IssuedCommand {
                        player: owner,
                        command: sim::Command::Interact {
                            unit: gi,
                            target: id,
                        },
                    });
                world.run_schedule(sim::SimSchedule);
                if hidden {
                    world
                        .resource_mut::<sim::SpecialState>()
                        .hidden
                        .insert((HUMAN, id));
                }
            });
            let id = house.expect("id");
            let cell = Some(CellPos::new(15, 15));
            assert!(
                host.world()
                    .resource::<sim::SpecialState>()
                    .garrisons
                    .contains_key(&id)
            );
            assert_eq!(own_building(&host, cell), (owner == HUMAN).then_some(id));
            assert_eq!(
                shown_enemy_building(&host, cell),
                (owner != HUMAN && !hidden).then_some(id)
            );
            assert_eq!(
                shown_scenery(&host, cell),
                None,
                "an occupied building is never neutral scenery"
            );
        }
    }

    /// RA1: a left click on the player's Construction Yard selects it (to pack it up,
    /// `crate::special_controls`); it has no rally point, so the cursor offers none and a right
    /// click orders nothing.
    #[test]
    fn a_click_on_the_construction_yard_selects_it_without_a_rally_point() {
        let host = crate::sim_host::tests::demo_host();
        let yards = own_yards(&host);
        assert_eq!(
            yards.len(),
            1,
            "the human player's yard, not the opponent's"
        );
        let yard = *yards.first().expect("a yard");
        assert!(!own_producers(&host).contains(&yard), "it produces no unit");
        let origin = {
            let mut buildings = host.world().try_query::<(&SimId, &Building)>().unwrap();
            buildings
                .iter(host.world())
                .find(|(id, _)| **id == yard)
                .map(|(_, building)| building.footprint.origin)
                .expect("the yard")
        };
        let (mut app, window, _) = input_app_on(host);
        app.add_systems(
            Update,
            (prune_selected_building, preview_order)
                .chain()
                .after(select_units)
                .before(issue_orders),
        );
        // Away from the unit, whose body is at the middle of the window.
        cursor(&mut app, window, Some(Vec2::new(300.0, 200.0)));
        app.world_mut().resource_mut::<HoveredCell>().0 = Some(origin);
        click(&mut app, MouseButton::Left);
        assert_eq!(
            *app.world().resource::<SelectedBuilding>(),
            SelectedBuilding(Some(yard))
        );
        assert!(app.world().resource::<Selection>().0.is_empty());
        // Over open ground: no order is offered, and a right click issues nothing.
        app.world_mut().resource_mut::<HoveredCell>().0 =
            Some(CellPos::new(origin.x + 6, origin.y + 5));
        app.update();
        assert_eq!(
            app.world().resource::<OrderPreview>().intent,
            OrderIntent::None
        );
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
        assert_eq!(app.world().resource::<CommandSink>().accepted(), 0);
        assert_eq!(
            app.world().resource::<SelectedBuilding>().0,
            Some(yard),
            "it stays selected"
        );
        // A click on open ground deselects it.
        click(&mut app, MouseButton::Left);
        assert_eq!(app.world().resource::<SelectedBuilding>().0, None);
        // The opponent's yard is not the player's to select.
        let host = app.world().resource::<SimHost>();
        let other = {
            let mut buildings = host
                .world()
                .try_query::<(&SimId, &Owner, &Building)>()
                .unwrap();
            buildings
                .iter(host.world())
                .find(|(_, owner, _)| owner.0 != HUMAN)
                .map(|(_, _, building)| building.footprint.origin)
                .expect("the opponent's yard")
        };
        app.world_mut().resource_mut::<HoveredCell>().0 = Some(other);
        click(&mut app, MouseButton::Left);
        assert_eq!(app.world().resource::<SelectedBuilding>().0, None);
    }

    /// A left click on a factory selects it, the cursor then offers a rally point, and a right
    /// click on the map sets that factory's rally point; selecting a unit ends it.
    #[test]
    fn a_selected_factory_takes_its_rally_point_from_a_right_click() {
        let mut placed = None;
        let host = crate::sim_host::tests::demo_host_with(|world| {
            // The first free site beside the human player's base.
            placed = (30..44)
                .flat_map(|y| (6..24).map(move |x| CellPos::new(x, y)))
                .find_map(|cell| {
                    sim::spawn_building(world, HUMAN, "barracks", cell)
                        .ok()
                        .map(|id| (id, cell))
                });
        });
        let (barracks, origin) = placed.expect("a site for the barracks");
        let (mut app, window, unit) = input_app_on(host);
        app.add_systems(
            Update,
            (prune_selected_building, preview_order)
                .chain()
                .after(select_units)
                .before(issue_orders),
        );
        // Away from the unit, whose body is at the middle of the window.
        cursor(&mut app, window, Some(Vec2::new(300.0, 200.0)));
        app.world_mut().resource_mut::<HoveredCell>().0 = Some(origin);
        click(&mut app, MouseButton::Left);
        assert_eq!(
            *app.world().resource::<SelectedBuilding>(),
            SelectedBuilding(Some(barracks))
        );
        assert!(app.world().resource::<Selection>().0.is_empty());
        // Over open ground the cursor offers the rally point.
        let rally = CellPos::new(origin.x + 6, origin.y + 5);
        app.world_mut().resource_mut::<HoveredCell>().0 = Some(rally);
        app.update();
        let preview = *app.world().resource::<OrderPreview>();
        assert_eq!(
            (preview.intent, preview.target),
            (OrderIntent::Rally, Some(barracks))
        );
        assert_eq!(app.world().resource::<CommandSink>().accepted(), 0);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
        assert_eq!(app.world().resource::<CommandSink>().accepted(), 1);
        assert_eq!(app.world().resource::<OrderFeedback>().cell, Some(rally));
        // The simulation accepts exactly that command.
        let mut host = app.world_mut().remove_resource::<SimHost>().expect("host");
        let events = crate::sim_host::tests::run(
            &mut host,
            rally_command(Some(barracks), Some(rally))
                .into_iter()
                .collect(),
        );
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, sim::SimEvent::CommandRejected { .. })),
            "{events:?}"
        );
        let mut producers = host.world().try_query::<(&SimId, &Producer)>().unwrap();
        assert_eq!(
            producers
                .iter(host.world())
                .find(|(id, _)| **id == barracks)
                .map(|(_, producer)| producer.rally),
            Some(Some(rally))
        );
        app.insert_resource(host);
        // A click on open ground deselects the factory; a click on a unit selects the unit.
        app.world_mut().resource_mut::<HoveredCell>().0 = Some(rally);
        click(&mut app, MouseButton::Left);
        assert_eq!(app.world().resource::<SelectedBuilding>().0, None);
        app.world_mut().resource_mut::<HoveredCell>().0 = Some(origin);
        click(&mut app, MouseButton::Left);
        assert_eq!(app.world().resource::<SelectedBuilding>().0, Some(barracks));
        cursor(&mut app, window, Some(Vec2::new(640.0, 400.0)));
        click(&mut app, MouseButton::Left);
        assert_eq!(app.world().resource::<SelectedBuilding>().0, None);
        assert_eq!(
            app.world().resource::<Selection>().0,
            BTreeSet::from([unit])
        );
        app.world_mut().resource_mut::<HoveredCell>().0 = Some(CellPos::new(20, 26));
        cursor(&mut app, window, Some(Vec2::new(300.0, 200.0)));
        app.update();
        assert_eq!(
            app.world().resource::<OrderPreview>().intent,
            OrderIntent::Move
        );
    }

    #[test]
    fn a_click_picks_a_unit_before_a_factory_and_a_factory_alone_offers_a_rally_point() {
        let (unit, factory) = (SimId(3), SimId(9));
        assert_eq!(
            click_pick(Some(unit), Some(factory)),
            (BTreeSet::from([unit]), None)
        );
        assert_eq!(
            click_pick(None, Some(factory)),
            (BTreeSet::new(), Some(factory))
        );
        assert_eq!(click_pick(None, None), (BTreeSet::new(), None));
        let cell = CellPos::new(4, 7);
        assert_eq!(
            rally_command(Some(factory), Some(cell)),
            Some(Command::SetRallyPoint {
                building: factory,
                cell
            })
        );
        assert_eq!(rally_command(None, Some(cell)), None);
        assert_eq!(rally_command(Some(factory), None), None);
        let facts = HoverFacts {
            over_map: true,
            producer: true,
            cell: Some(cell),
            ..default()
        };
        assert_eq!(order_intent(facts), OrderIntent::Rally);
        assert_eq!(OrderIntent::Rally.label(), "RALLY POINT");
        // Off the map there is nowhere to rally; over the sidebar nothing is offered; and
        // selected units take the click for themselves.
        assert_eq!(
            order_intent(HoverFacts {
                cell: None,
                ..facts
            }),
            OrderIntent::Invalid
        );
        assert_eq!(
            order_intent(HoverFacts {
                over_map: false,
                ..facts
            }),
            OrderIntent::None
        );
        assert_eq!(
            order_intent(HoverFacts {
                producer: false,
                ..facts
            }),
            OrderIntent::None
        );
        assert_eq!(
            order_intent(HoverFacts {
                selected: 2,
                producer: false,
                enterable: true,
                ..facts
            }),
            OrderIntent::Move
        );
    }

    #[test]
    fn production_selection_uses_visible_body_and_keeps_last_drag_endpoint() {
        let (mut app, window, id) = input_app();
        cursor(&mut app, window, Some(Vec2::new(620.0, 390.0)));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        cursor(&mut app, window, Some(Vec2::new(660.0, 410.0)));
        app.update();
        cursor(&mut app, window, None);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        assert_eq!(app.world().resource::<Selection>().0, BTreeSet::from([id]));
        // Panning gesture cannot begin a box selection or clear the current selection.
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        cursor(&mut app, window, Some(Vec2::new(300.0, 300.0)));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(app.world().resource::<DragBox>().start.is_none());
        assert!(app.world().resource::<Selection>().0.contains(&id));
    }
    #[test]
    fn production_order_input_ignores_sidebar_and_acknowledges_battlefield_clicks() {
        let (mut app, window, id) = input_app();
        app.world_mut().resource_mut::<Selection>().0.insert(id);
        app.world_mut().resource_mut::<HoveredCell>().0 = Some(CellPos::new(20, 26));
        cursor(&mut app, window, Some(Vec2::new(1200.0, 400.0)));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.update();
        assert_eq!(app.world().resource::<CommandSink>().accepted(), 0);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
        cursor(&mut app, window, Some(Vec2::new(600.0, 400.0)));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.update();
        assert_eq!(app.world().resource::<CommandSink>().accepted(), 1);
        assert_eq!(
            app.world().resource::<OrderFeedback>().cell,
            Some(CellPos::new(20, 26))
        );
    }
    #[test]
    fn the_cursor_turns_red_over_an_enemy_before_the_click_and_refuses_blocked_ground() {
        let (mut app, window, id) = input_app();
        app.add_systems(
            Update,
            (preview_order, show_intent_cursor)
                .chain()
                .before(issue_orders)
                .after(select_units),
        )
        .add_systems(Startup, spawn_intent_label);
        // One tick computes the player's vision, so the base's surroundings are explored.
        crate::sim_host::tests::run(&mut app.world_mut().resource_mut::<SimHost>(), vec![]);
        // A visible enemy unit 200 world units right of the player's unit, and the map as the
        // interface shows it, with a rock cell next to the base.
        let (enemy, enemy_id, enemy_pos) = {
            let host = app.world().resource::<SimHost>();
            let mut q = host
                .world()
                .try_query::<(Entity, &SimId, &Owner, &sim::Position)>()
                .unwrap();
            let (e, i, _, p) = q
                .iter(host.world())
                .find(|(_, _, o, _)| o.0 != HUMAN)
                .unwrap();
            (e, *i, p.0)
        };
        let mut terrain = app.world().resource::<SimHost>().grid().unwrap().clone();
        let rock = CellPos::new(19, 38);
        terrain.set_terrain(
            rock,
            sim::Cell {
                passable: false,
                buildable: false,
                water: false,
                ore: 0,
            },
        );
        app.insert_resource(MapTerrain(terrain));
        app.world_mut().spawn((
            UnitView {
                unit: enemy,
                id: enemy_id,
                owner: sim::PlayerId(1),
                previous: enemy_pos,
                current: enemy_pos,
                altitude: 0.0,
                drawn: None,
                stride: 0.0,
            },
            Transform::from_xyz(200.0, 0.0, 10.0),
            Visibility::Visible,
        ));
        app.world_mut().resource_mut::<Selection>().0.insert(id);
        // Hovering the enemy: the preview is a red ATTACK before any click.
        app.world_mut().resource_mut::<HoveredCell>().0 = Some(CellPos::new(30, 30));
        cursor(&mut app, window, Some(Vec2::new(840.0, 400.0)));
        app.update();
        let preview = *app.world().resource::<OrderPreview>();
        assert_eq!(preview.intent, OrderIntent::Attack);
        assert_eq!(preview.target, Some(enemy_id));
        let (text, colour) = app
            .world_mut()
            .query_filtered::<(&Text, &TextColor), With<IntentLabel>>()
            .single(app.world())
            .map(|(t, c)| (t.0.clone(), c.0))
            .unwrap();
        assert_eq!(text, "ATTACK");
        assert_eq!(colour, OrderIntent::Attack.colour());
        assert_eq!(
            app.world().get::<bevy::window::CursorIcon>(window),
            Some(&bevy::window::CursorIcon::System(
                bevy::window::SystemCursorIcon::Crosshair
            ))
        );
        assert_eq!(app.world().resource::<CommandSink>().accepted(), 0);
        // The click issues the attack it showed.
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.update();
        assert_eq!(app.world().resource::<CommandSink>().accepted(), 1);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
        // Explored rock beside the base: CANNOT GO, and a click orders nothing.
        app.world_mut().resource_mut::<HoveredCell>().0 = Some(rock);
        cursor(&mut app, window, Some(Vec2::new(600.0, 600.0)));
        app.update();
        assert_eq!(
            app.world().resource::<OrderPreview>().intent,
            OrderIntent::Invalid
        );
        assert_eq!(
            app.world().get::<bevy::window::CursorIcon>(window),
            Some(&bevy::window::CursorIcon::System(
                bevy::window::SystemCursorIcon::NotAllowed
            ))
        );
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.update();
        assert_eq!(app.world().resource::<CommandSink>().accepted(), 1);
        // Ordinary ground next to it: a green MOVE.
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
        app.world_mut().resource_mut::<HoveredCell>().0 = Some(CellPos::new(20, 38));
        app.update();
        assert_eq!(
            app.world().resource::<OrderPreview>().intent,
            OrderIntent::Move
        );
    }

    fn ids(values: &[u32]) -> BTreeSet<SimId> {
        values.iter().map(|&id| SimId(id)).collect()
    }

    const UNITS: [(SimId, Vec2); 3] = [
        (SimId(1), Vec2::new(0.0, 0.0)),
        (SimId(2), Vec2::new(30.0, 0.0)),
        (SimId(3), Vec2::new(0.0, -100.0)),
    ];

    #[test]
    fn a_click_selects_the_unit_whose_body_is_under_the_cursor() {
        let body = Vec2::new(0.0, GROUND_OFFSET_PX);
        assert_eq!(unit_at(body, &UNITS), Some(SimId(1)));
        // The edges of the pick area are included, just beyond them nothing is picked.
        assert_eq!(
            unit_at(body + Vec2::new(-20.0, 16.0), &UNITS),
            Some(SimId(1))
        );
        assert_eq!(unit_at(body + Vec2::new(-20.5, 0.0), &UNITS), None);
        assert_eq!(unit_at(body + Vec2::new(0.0, 16.5), &UNITS), None);
        // Overlapping pick areas: the nearer body wins, and on a tie the lower id.
        assert_eq!(unit_at(body + Vec2::new(14.0, 0.0), &UNITS), Some(SimId(1)));
        assert_eq!(unit_at(body + Vec2::new(16.0, 0.0), &UNITS), Some(SimId(2)));
        assert_eq!(unit_at(body + Vec2::new(15.0, 0.0), &UNITS), Some(SimId(1)));
        // Empty ground, and no units at all.
        assert_eq!(unit_at(Vec2::new(500.0, 500.0), &UNITS), None);
        assert_eq!(unit_at(body, &[]), None);
    }

    #[test]
    fn a_box_selects_the_units_whose_ground_point_lies_inside() {
        let both = units_in_box(Vec2::new(-5.0, -5.0), Vec2::new(35.0, 5.0), &UNITS);
        assert_eq!(both, ids(&[1, 2]));
        // Corners in any order, edges included.
        let same = units_in_box(Vec2::new(30.0, 0.0), Vec2::new(0.0, -100.0), &UNITS);
        assert_eq!(same, ids(&[1, 2, 3]));
        assert_eq!(
            units_in_box(Vec2::new(1.0, 1.0), Vec2::new(29.0, 50.0), &UNITS),
            ids(&[])
        );
    }

    #[test]
    fn shift_adds_to_the_selection_and_otherwise_replaces_it() {
        let current = ids(&[1, 2]);
        assert_eq!(apply_selection(&current, ids(&[3]), false), ids(&[3]));
        assert_eq!(apply_selection(&current, ids(&[3]), true), ids(&[1, 2, 3]));
        assert_eq!(apply_selection(&current, ids(&[2]), true), ids(&[1, 2]));
        // Clicking empty ground clears the selection, unless Shift is held.
        assert_eq!(apply_selection(&current, ids(&[]), false), ids(&[]));
        assert_eq!(apply_selection(&current, ids(&[]), true), current);
    }

    #[test]
    fn game_systems_that_share_state_in_a_frame_are_ordered() {
        // M2 review 1, P2-3: `issue_orders` reads the hovered cell that `update_hovered_cell`
        // writes in the same schedule; unordered, a right click could use the previous frame's
        // cell. Bevy records every pair of systems of a schedule that have conflicting access
        // and no ordering; the game's own systems in `PostUpdate` must form no such pair.
        use bevy::ecs::system::IntoSystem;
        use std::any::TypeId;
        fn type_of<M>(system: impl IntoSystem<(), (), M>) -> TypeId {
            IntoSystem::into_system(system).system_type()
        }
        // Without Bevy's `debug` feature systems have no names, so they are told apart by type.
        let known = [
            (
                type_of(crate::map::update_hovered_cell),
                "update_hovered_cell",
            ),
            (type_of(issue_orders), "issue_orders"),
            (type_of(select_units), "select_units"),
            (type_of(prune_selection), "prune_selection"),
            (type_of(draw_selection), "draw_selection"),
        ];
        let label = |id: TypeId| {
            known
                .iter()
                .find(|(known, _)| *known == id)
                .map_or("another game system", |(_, name)| name)
        };
        let mut app = App::new();
        app.add_plugins((
            crate::map::MapPlugin,
            crate::units::UnitsPlugin,
            SelectionPlugin,
        ));
        app.world_mut()
            .schedule_scope(PostUpdate, |world, schedule| {
                schedule.initialize(world).expect("PostUpdate builds");
                let types: std::collections::BTreeMap<_, TypeId> = schedule
                    .systems()
                    .expect("initialized")
                    .map(|(key, system)| (key, system.system_type()))
                    .collect();
                for (_, name) in &known {
                    assert!(
                        types.values().any(|&id| label(id) == *name),
                        "{name} is not in PostUpdate"
                    );
                }
                let conflicts: Vec<(&str, &str)> = schedule
                    .graph()
                    .conflicting_systems()
                    .iter()
                    .map(|(a, b, _)| (label(types[a]), label(types[b])))
                    .collect();
                assert_eq!(conflicts, Vec::<(&str, &str)>::new());
            });
    }

    #[test]
    fn a_right_click_on_ore_sends_the_selected_harvesters_to_harvest() {
        let selection = ids(&[1, 2, 3]);
        let harvesters = ids(&[2, 3, 8]);
        let target = CellPos::new(5, 6);
        assert_eq!(
            right_click_commands(&selection, &harvesters, Some(target), true),
            [
                Command::Harvest {
                    units: vec![SimId(2), SimId(3)],
                    target
                },
                Command::Move {
                    units: vec![SimId(1)],
                    target
                }
            ]
        );
        // Only harvesters selected: no move.
        assert_eq!(
            right_click_commands(&ids(&[3]), &harvesters, Some(target), true),
            [Command::Harvest {
                units: vec![SimId(3)],
                target
            }]
        );
        // No ore there, or no harvester selected: the whole selection moves.
        for (selection, ore) in [(ids(&[1, 2, 3]), false), (ids(&[1]), true)] {
            assert_eq!(
                right_click_commands(&selection, &harvesters, Some(target), ore),
                move_command(&selection, Some(target))
                    .into_iter()
                    .collect::<Vec<_>>()
            );
        }
        assert_eq!(
            right_click_commands(&selection, &harvesters, None, true),
            []
        );
        assert_eq!(
            right_click_commands(&ids(&[]), &harvesters, Some(target), true),
            []
        );
    }

    #[test]
    fn no_two_game_systems_share_state_in_post_update_without_an_order() {
        // As above, with every game plugin that adds systems to `PostUpdate` (M3 adds the
        // buildings' views and the economy controls, which push commands like the orders).
        let mut app = App::new();
        app.add_plugins((
            crate::map::MapPlugin,
            crate::units::UnitsPlugin,
            crate::buildings::BuildingsPlugin,
            SelectionPlugin,
            crate::construction::ConstructionPlugin,
        ));
        app.world_mut()
            .schedule_scope(PostUpdate, |world, schedule| {
                schedule.initialize(world).expect("PostUpdate builds");
                assert!(schedule.graph().conflicting_systems().is_empty());
            });
    }

    #[test]
    fn the_preview_tells_attack_move_harvest_friendly_and_invalid_apart_before_the_click() {
        let base = HoverFacts {
            over_map: true,
            selected: 2,
            armed: true,
            cell: Some(CellPos::new(5, 5)),
            enterable: true,
            ..HoverFacts::default()
        };
        assert_eq!(order_intent(base), OrderIntent::Move);
        let enemy = HoverFacts {
            enemy: Some(SimId(9)),
            ..base
        };
        assert_eq!(order_intent(enemy), OrderIntent::Attack);
        assert_eq!(OrderIntent::Attack.colour(), Color::srgb(1.0, 0.12, 0.08));
        // Unarmed units move next to an enemy instead: no red attack marker.
        assert_eq!(
            order_intent(HoverFacts {
                armed: false,
                ..enemy
            }),
            OrderIntent::Move
        );
        // An enemy wins over the terrain or ore under it.
        assert_eq!(
            order_intent(HoverFacts {
                ore: true,
                harvesters: true,
                enterable: false,
                ..enemy
            }),
            OrderIntent::Attack
        );
        let ore = HoverFacts {
            ore: true,
            harvesters: true,
            ..base
        };
        assert_eq!(order_intent(ore), OrderIntent::Harvest);
        assert_eq!(
            order_intent(HoverFacts {
                harvesters: false,
                ..ore
            }),
            OrderIntent::Move
        );
        assert_eq!(
            order_intent(HoverFacts {
                friendly: Some(SimId(3)),
                ..base
            }),
            OrderIntent::Friendly
        );
        // Water or rock none of the selected units can enter, or off the map: nothing happens.
        assert_eq!(
            order_intent(HoverFacts {
                enterable: false,
                ..base
            }),
            OrderIntent::Invalid
        );
        assert_eq!(
            order_intent(HoverFacts { cell: None, ..base }),
            OrderIntent::Invalid
        );
        // No selection, or off the battlefield: no preview at all.
        assert_eq!(
            order_intent(HoverFacts {
                selected: 0,
                ..enemy
            }),
            OrderIntent::None
        );
        assert_eq!(
            order_intent(HoverFacts {
                over_map: false,
                ..enemy
            }),
            OrderIntent::None
        );
        // Every intent has its own colour and label; attack alone is red.
        let all = [
            OrderIntent::Move,
            OrderIntent::Attack,
            OrderIntent::Harvest,
            OrderIntent::Friendly,
            OrderIntent::Invalid,
        ];
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a.colour(), b.colour());
                assert_ne!(a.label(), b.label());
            }
            let red = a.colour().to_srgba();
            assert_eq!(
                red.red > 0.9 && red.green < 0.3 && red.blue < 0.3,
                *a == OrderIntent::Attack,
                "{a:?}"
            );
        }
    }

    #[test]
    fn a_right_click_on_a_visible_enemy_orders_the_armed_units_to_attack_it() {
        let selection = ids(&[1, 2, 3]);
        let armed = ids(&[1, 3, 9]);
        let target = SimId(40);
        let cell = Some(CellPos::new(4, 4));
        assert_eq!(
            attack_commands(&selection, &armed, target, cell),
            [
                Command::Attack {
                    units: vec![SimId(1), SimId(3)],
                    target
                },
                Command::Move {
                    units: vec![SimId(2)],
                    target: CellPos::new(4, 4)
                }
            ]
        );
        // Only unarmed units selected: they move; outside the map they do nothing.
        assert_eq!(
            attack_commands(&ids(&[2]), &armed, target, cell),
            move_command(&ids(&[2]), cell)
                .into_iter()
                .collect::<Vec<_>>()
        );
        assert_eq!(
            attack_commands(&ids(&[1]), &armed, target, None),
            [Command::Attack {
                units: vec![SimId(1)],
                target
            }]
        );
        assert_eq!(attack_commands(&ids(&[]), &armed, target, cell), []);
        // The enemy under the cursor: a unit body first, else the building under the cursor.
        let body = Vec2::new(0.0, GROUND_OFFSET_PX);
        let enemies = [(SimId(40), Vec2::ZERO)];
        assert_eq!(enemy_at(body, &enemies, Some(SimId(50))), Some(SimId(40)));
        assert_eq!(
            enemy_at(Vec2::new(500.0, 0.0), &enemies, Some(SimId(50))),
            Some(SimId(50))
        );
        assert_eq!(enemy_at(Vec2::new(500.0, 0.0), &enemies, None), None);
        assert_eq!(enemy_at(body, &[], None), None);
    }

    #[test]
    fn the_attack_move_key_orders_the_whole_selection() {
        let selection = ids(&[5, 2]);
        let target = CellPos::new(9, 1);
        assert_eq!(
            attack_move_command(&selection, Some(target)),
            Some(Command::AttackMove {
                units: vec![SimId(2), SimId(5)],
                target
            })
        );
        assert_eq!(attack_move_command(&selection, None), None);
        assert_eq!(attack_move_command(&ids(&[]), Some(target)), None);
    }

    #[test]
    fn orders_are_commands_for_the_whole_selection() {
        let selection = ids(&[4, 2, 9]);
        let target = CellPos::new(7, 3);
        assert_eq!(
            move_command(&selection, Some(target)),
            Some(Command::Move {
                units: vec![SimId(2), SimId(4), SimId(9)],
                target
            })
        );
        assert_eq!(
            stop_command(&selection),
            Some(Command::Stop {
                units: vec![SimId(2), SimId(4), SimId(9)]
            })
        );
        // Nothing is ordered outside the map or without a selection.
        assert_eq!(move_command(&selection, None), None);
        assert_eq!(move_command(&ids(&[]), Some(target)), None);
        assert_eq!(stop_command(&ids(&[])), None);
    }
    // Historical preparation note: UNCOMPILED / UNRUN / UNACCEPTED. Append INSIDE existing selection::tests.
    // Actual command-created SimHost actors are independent of controlled UnitView layout.
    // Transform/Visibility/ComputedCamera/HoveredCell below are input-fixture presentation only;
    // they do not spawn actors, relations, command receipts, visibility or source flag readers.
    fn c62_view_data(host: &SimHost, id: SimId) -> (Entity, sim::SubPos, sim::PlayerId) {
        let mut q = host
            .world()
            .try_query::<(Entity, &SimId, &sim::Owner, &sim::Position)>()
            .unwrap();
        q.iter(host.world())
            .find(|(_, found, ..)| **found == id)
            .map(|(e, _, o, p)| (e, p.0, o.0))
            .unwrap()
    }
    fn c62_input(host: SimHost, subject: Option<SimId>, normal: SimId) -> (App, Entity) {
        assert!(
            host.running(),
            "real setup_match_configured, not a fabricated running state"
        );
        let grid = host.grid().unwrap().clone();
        let mut data = vec![(normal, c62_view_data(&host, normal), 200.0)];
        if let Some(id) = subject {
            data.push((id, c62_view_data(&host, id), 0.0));
        }
        let (mut app, window, _) = input_app_on(host);
        let old = app
            .world_mut()
            .query_filtered::<Entity, With<UnitView>>()
            .iter(app.world())
            .collect::<Vec<_>>();
        for e in old {
            app.world_mut().despawn(e);
        }
        for (id, (unit, pos, owner), x) in data {
            app.world_mut().spawn((
                UnitView {
                    unit,
                    id,
                    owner,
                    previous: pos,
                    current: pos,
                    altitude: 0.0,
                    drawn: None,
                    stride: 0.0,
                },
                Transform::from_xyz(x, 0.0, 10.0),
                Visibility::Visible,
            ));
        }
        app.init_resource::<ControlGroups>()
            .insert_resource(crate::map::MapTerrain(grid))
            .insert_resource(crate::map::MapView {
                width: 48,
                height: 48,
                tile: crate::projection::TileSize::new(64, 32),
            })
            .add_message::<sim::SimEvent>()
            .add_message::<AppExit>()
            .add_systems(
                Update,
                preview_order.after(select_units).before(issue_orders),
            );
        (app, window)
    }
    fn c62_select_normal(app: &mut App, window: Entity) {
        app.world_mut().resource_mut::<HoveredCell>().0 = None;
        cursor(app, window, Some(Vec2::new(840.0, 400.0)));
        click(app, MouseButton::Left);
    }
    fn c62_empty_subject_click(app: &mut App, window: Entity) {
        app.world_mut().resource_mut::<HoveredCell>().0 = None;
        cursor(app, window, Some(Vec2::new(640.0, 400.0)));
        click(app, MouseButton::Left);
    }
    fn c62_hover(app: &mut App, window: Entity, cell: CellPos) {
        app.world_mut().resource_mut::<HoveredCell>().0 = Some(cell);
        cursor(app, window, Some(Vec2::new(800.0, 600.0)));
        app.update();
    }
    fn c62_pending(app: &App) -> Vec<Command> {
        crate::sim_host::tests::pending_commands_for_test(app.world().resource::<CommandSink>())
    }
    fn c62_key(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        crate::ra7_controls::run_extended_keys_for_test(app.world_mut());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
    }
    fn c62_cycle_from_normal(
        app: &mut App,
        window: Entity,
        normal: SimId,
        excluded: SimId,
        eligible: &[SimId],
    ) {
        c62_select_normal(app, window);
        assert_eq!(
            app.world().resource::<Selection>().0,
            BTreeSet::from([normal])
        );
        let n = eligible.iter().position(|id| *id == normal).unwrap();
        c62_key(app, KeyCode::KeyN);
        assert_eq!(
            app.world().resource::<Selection>().0,
            BTreeSet::from([eligible[(n + 1) % eligible.len()]])
        );
        c62_key(app, KeyCode::KeyM);
        assert_eq!(
            app.world().resource::<Selection>().0,
            BTreeSet::from([normal])
        );
        let mut seen = BTreeSet::new();
        for _ in 0..eligible.len() {
            c62_key(app, KeyCode::KeyN);
            let selected = &app.world().resource::<Selection>().0;
            assert_eq!(selected.len(), 1);
            assert!(!selected.contains(&excluded));
            seen.extend(selected.iter().copied());
        }
        assert_eq!(seen, eligible.iter().copied().collect());
        assert_eq!(app.world().resource::<CommandSink>().accepted(), 0);
        assert!(
            c62_pending(app).is_empty(),
            "selection/cycling must not issue gameplay commands"
        );
    }
    fn c62_mission_selection_and_sink(section: &str) {
        let (host, subject, normal, _) =
            crate::minimap::ra9_behavior87_routes::c62_mission(section);
        let mut q = host.world().try_query::<(&SimId, &sim::Mobile)>().unwrap();
        let before = q
            .iter(host.world())
            .find(|(id, _)| **id == subject)
            .unwrap()
            .1
            .sub_position();
        let (mut app, window) = c62_input(host, Some(subject), normal);
        c62_empty_subject_click(&mut app, window);
        assert!(app.world().resource::<Selection>().0.is_empty());
        c62_empty_subject_click(&mut app, window); // Second click cannot type-select a real mission.
        assert!(app.world().resource::<Selection>().0.is_empty());
        assert_eq!(app.world().resource::<SelectedBuilding>().0, None);
        c62_select_normal(&mut app, window);
        assert_eq!(
            app.world().resource::<Selection>().0,
            BTreeSet::from([normal])
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ShiftLeft);
        c62_empty_subject_click(&mut app, window);
        assert_eq!(
            app.world().resource::<Selection>().0,
            BTreeSet::from([normal])
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        cursor(&mut app, window, Some(Vec2::new(600.0, 350.0)));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        cursor(&mut app, window, Some(Vec2::new(880.0, 450.0)));
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
        assert_eq!(
            app.world().resource::<Selection>().0,
            BTreeSet::from([normal])
        );
        c62_empty_subject_click(&mut app, window);
        let dest = CellPos::new(20, 14);
        c62_hover(&mut app, window, dest);
        assert_eq!(
            app.world().resource::<OrderPreview>().intent,
            OrderIntent::None
        );
        click(&mut app, MouseButton::Right);
        assert_eq!(app.world().resource::<CommandSink>().accepted(), 0);
        assert!(
            c62_pending(&app).is_empty(),
            "unselected mission cannot reach CommandSink"
        );
        c62_select_normal(&mut app, window);
        c62_hover(&mut app, window, dest);
        assert_eq!(
            app.world().resource::<OrderPreview>().intent,
            OrderIntent::Move
        );
        click(&mut app, MouseButton::Right);
        let expected = Command::Move {
            units: vec![normal],
            target: dest,
        };
        assert_eq!(c62_pending(&app), vec![expected]);
        assert_eq!(app.world().resource::<CommandSink>().accepted(), 1);
        let tick = app.world().resource::<SimHost>().tick();
        crate::sim_host::tests::run_sink_tick_for_test(app.world_mut());
        let host = app.world().resource::<SimHost>();
        assert_eq!(host.tick(), tick + 1);
        let events = app
            .world()
            .resource::<bevy::ecs::message::Messages<sim::SimEvent>>();
        assert!(
            !events
                .iter_current_update_messages()
                .any(|e| matches!(e, sim::SimEvent::CommandRejected { .. }))
        );
        let mut q = host
            .world()
            .try_query::<(&SimId, &sim::Mobile, &sim::Orders)>()
            .unwrap();
        let (_, mobile, _) = q
            .iter(host.world())
            .find(|(id, ..)| **id == subject)
            .unwrap();
        assert_ne!(
            mobile.sub_position(),
            before,
            "real internal mission continues during ordinary command"
        );
        let (_, _, orders) = q
            .iter(host.world())
            .find(|(id, ..)| **id == normal)
            .unwrap();
        assert_eq!(orders.current.as_ref().map(|o| o.destination), Some(dest));
        assert!(
            c62_pending(&app).is_empty(),
            "actual run_tick consumed the real sink batch"
        );
    }
    fn c62_mission_cycles(section: &str) {
        let (host, subject, normal, eligible) =
            crate::minimap::ra9_behavior87_routes::c62_mission(section);
        let (mut app, window) = c62_input(host, Some(subject), normal);
        c62_cycle_from_normal(&mut app, window, normal, subject, &eligible);
        // A full native cycle returns to normal. A second immediate fixture click would
        // instead exercise double-click/frustum selection (Time has not advanced).
        assert_eq!(
            app.world().resource::<Selection>().0,
            BTreeSet::from([normal])
        );
        c62_key(&mut app, KeyCode::KeyT);
        assert_eq!(
            app.world().resource::<Selection>().0,
            if section == "PDPLANE" {
                eligible.iter().copied().collect()
            } else {
                BTreeSet::from([normal])
            }
        );
        c62_key(&mut app, KeyCode::KeyP);
        let combat = eligible
            .iter()
            .copied()
            .filter(|id| *id != normal)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            app.world().resource::<Selection>().0,
            if section == "PDPLANE" {
                BTreeSet::new()
            } else {
                combat
            }
        );
        assert!(!app.world().resource::<Selection>().0.contains(&subject));
        assert_eq!(app.world().resource::<CommandSink>().accepted(), 0);
    }
    #[test]
    fn ra9_c62_dmisl_real_click_box_preview_sink() {
        c62_mission_selection_and_sink("DMISL");
    }
    #[test]
    fn ra9_c62_v3_real_click_box_preview_sink() {
        c62_mission_selection_and_sink("V3ROCKET");
    }
    #[test]
    fn ra9_c62_pdplane_real_click_box_preview_sink() {
        c62_mission_selection_and_sink("PDPLANE");
    }
    #[test]
    fn ra9_c62_dmisl_real_cycle_type_combat() {
        c62_mission_cycles("DMISL");
    }
    #[test]
    fn ra9_c62_v3_real_cycle_type_combat() {
        c62_mission_cycles("V3ROCKET");
    }
    #[test]
    fn ra9_c62_pdplane_real_cycle_type_combat() {
        c62_mission_cycles("PDPLANE");
    }

    /// Exact40 source aliases share one current-policy/input/native-route loop.
    /// Manual native spawns are ordinary scenario actors, not production/mission proof.
    /// UnitView x200 and HoveredCell are controlled mouse fixtures; cycle/T/P read real
    /// SimHost components. The two source false Engineer rows still select/group/move.
    #[test]
    fn ra9_b87_all40_stock_aliases_use_real_group_cycle_type_combat_preview_and_sink() {
        let cases: [(&str, &str, bool); 40] = [
            ("ADOG", "attack_dog", true),
            ("AEGIS", "aegis_cruiser", true),
            ("APOC", "apocalypse", true),
            ("BEAG", "black_eagle", true),
            ("CARRIER", "aircraft_carrier", true),
            ("CCOMAND", "chrono_commando", true),
            ("CIVAN", "chrono_ivan", true),
            ("CLEG", "chrono_legionnaire", true),
            ("DESO", "desolator", true),
            ("DEST", "destroyer", true),
            ("DLPH", "dolphin", true),
            ("DOG", "attack_dog", true),
            ("DRED", "dreadnought", true),
            ("DRON", "terror_drone", true),
            ("E1", "gi", true),
            ("E2", "conscript", true),
            ("ENGINEER", "engineer", false),
            ("FLAKT", "flak_trooper", true),
            ("FV", "ifv", true),
            ("HTK", "flak_track", true),
            ("HTNK", "rhino", true),
            ("HYD", "sea_scorpion", true),
            ("IVAN", "crazy_ivan", true),
            ("JUMPJET", "rocketeer", true),
            ("MGTK", "mirage_tank", true),
            ("MTNK", "grizzly", true),
            ("ORCA", "harrier", true),
            ("PTROOP", "psi_commando", true),
            ("SENGINEER", "engineer", false),
            ("SHK", "tesla_trooper", true),
            ("SNIPE", "sniper", true),
            ("SQD", "giant_squid", true),
            ("SREF", "prism_tank", true),
            ("TANY", "tanya", true),
            ("TNKD", "tank_destroyer", true),
            ("TTNK", "tesla_tank", true),
            ("V3", "v3_launcher", true),
            ("YURI", "yuri", true),
            ("YURIPR", "yuri_prime", true),
            ("ZEP", "kirov", true),
        ];
        let contracts = include_str!("../../sim/tests/ra9_audit/behavior87-contracts.tsv");
        let expected_source: BTreeSet<_> = cases
            .iter()
            .map(|(section, _, combat)| (*section, if *combat { "yes" } else { "no" }))
            .collect();
        let actual_source: BTreeSet<_> = contracts
            .lines()
            .skip(1)
            .filter_map(|line| {
                let c: Vec<_> = line.split('\t').collect();
                (c[4] == "combat-selection").then_some((c[1], c[3]))
            })
            .collect();
        assert_eq!(
            actual_source, expected_source,
            "exact40 source identities/values; no runtime flag credit"
        );
        assert_eq!(expected_source.len(), 40);
        let mapping = include_str!("../../sim/tests/reference/ra2_mapping.tsv");
        for (section, name, _) in cases {
            assert_eq!(
                mapping
                    .lines()
                    .skip(1)
                    .filter(|line| {
                        let c: Vec<_> = line.split('\t').collect();
                        c[0] == name && c[1] == "unit" && c[2] == section
                    })
                    .count(),
                1,
                "{section}: independent committed mapping"
            );
        }
        let mut actors = Vec::new();
        let mut foreign = None;
        let host = crate::minimap::ra9_behavior87_routes::c62_running_stock_host(30, |w| {
            sim::spawn_building(w, HUMAN, "airforce_command", CellPos::new(1, 1)).unwrap();
            let d = w.resource::<sim::Definitions>().clone();
            let mut land = 0;
            let mut sea = 0;
            let mut air = 0;
            for (section, name, combat) in cases {
                for _ in 0..2 {
                    let kind = d.unit_kind(name).unwrap();
                    let (x, y) = match d.domain(kind) {
                        sim::Domain::Naval => {
                            let n = sea;
                            sea += 1;
                            (34 + (n % 5) * 2, 8 + (n / 5) * 2)
                        }
                        sim::Domain::Air => {
                            let n = air;
                            air += 1;
                            (8 + (n % 10) * 2, 8 + (n / 10) * 2)
                        }
                        _ => {
                            let n = land;
                            land += 1;
                            (8 + (n % 10) * 2, 8 + (n / 10) * 2)
                        }
                    };
                    let id = sim::spawn_unit(w, HUMAN, name, CellPos::new(x, y)).unwrap();
                    actors.push((section, name, combat, id));
                }
            }
            foreign = Some(
                sim::spawn_unit(w, sim::PlayerId(1), "grizzly", CellPos::new(45, 46)).unwrap(),
            );
        });
        assert!(host.running());
        let foreign = foreign.unwrap();
        assert_eq!(actors.len(), 80);
        let own: BTreeSet<_> = actors.iter().map(|a| a.3).collect();
        let combat: BTreeSet<_> = actors.iter().filter(|a| a.2).map(|a| a.3).collect();
        assert_eq!(own.len(), 80);
        assert_eq!(combat.len(), 76);
        let first = actors[0].3;
        let (mut app, window) = c62_input(host, None, first);
        app.add_systems(Update, control_groups.before(select_units));
        for (index, (section, name, _, id)) in actors.iter().step_by(2).enumerate() {
            let (unit, pos, owner) = c62_view_data(app.world().resource::<SimHost>(), *id);
            assert_eq!(owner, HUMAN, "{section}");
            {
                let mut q = app.world_mut().query::<&mut UnitView>();
                let mut view = q.single_mut(app.world_mut()).unwrap();
                *view = UnitView {
                    unit,
                    id: *id,
                    owner,
                    previous: pos,
                    current: pos,
                    altitude: 0.0,
                    drawn: None,
                    stride: 0.0,
                };
            }
            // Independent single-click trials; do not accidentally invoke the separate
            // double-click frustum branch of the controlled UnitView layout.
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs(1));
            c62_select_normal(&mut app, window);
            assert_eq!(
                app.world().resource::<Selection>().0,
                BTreeSet::from([*id]),
                "{section}: ordinary click, including false Engineer"
            );
            // Actual drag input, not a direct Selection assignment.
            cursor(&mut app, window, Some(Vec2::new(800.0, 350.0)));
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Left);
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .clear();
            cursor(&mut app, window, Some(Vec2::new(880.0, 450.0)));
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .release(MouseButton::Left);
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .reset_all();
            assert_eq!(
                app.world().resource::<Selection>().0,
                BTreeSet::from([*id]),
                "{section}: ordinary box"
            );
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::ControlLeft);
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::Digit1);
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .reset_all();
            assert_eq!(
                app.world().resource::<ControlGroups>().0[1],
                BTreeSet::from([*id])
            );
            c62_empty_subject_click(&mut app, window);
            assert!(app.world().resource::<Selection>().0.is_empty());
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::Digit1);
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .reset_all();
            assert_eq!(
                app.world().resource::<Selection>().0,
                BTreeSet::from([*id]),
                "{section}: real group recall"
            );
            c62_key(&mut app, KeyCode::KeyN);
            let next = own
                .range((std::ops::Bound::Excluded(*id), std::ops::Bound::Unbounded))
                .next()
                .or_else(|| own.first())
                .unwrap();
            assert_eq!(
                app.world().resource::<Selection>().0,
                BTreeSet::from([*next]),
                "{section}: real next"
            );
            c62_key(&mut app, KeyCode::KeyM);
            assert_eq!(
                app.world().resource::<Selection>().0,
                BTreeSet::from([*id]),
                "{section}: real previous"
            );
            c62_key(&mut app, KeyCode::KeyT);
            let same_kind = actors
                .iter()
                .filter(|a| a.1 == *name)
                .map(|a| a.3)
                .collect::<BTreeSet<_>>();
            assert!(same_kind.len() >= 2);
            assert_eq!(
                app.world().resource::<Selection>().0,
                same_kind,
                "{section}: real type, including shared ADOG/DOG and ENGINEER/SENGINEER kinds"
            );
            c62_key(&mut app, KeyCode::KeyP);
            assert_eq!(
                app.world().resource::<Selection>().0,
                combat,
                "{section}: actual current combat policy"
            );
            assert!(!app.world().resource::<Selection>().0.contains(&foreign));
            assert_eq!(
                app.world().resource::<CommandSink>().accepted(),
                index as u64,
                "selection/group/type/combat must issue no gameplay commands"
            );
            assert!(c62_pending(&app).is_empty());
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs(1));
            c62_select_normal(&mut app, window);
            let domain = app
                .world()
                .resource::<SimHost>()
                .grid()
                .unwrap()
                .domain_of(*id);
            // Earlier native Move trials may already occupy the former fixed destination.
            // Pick empty legal fixture geometry; do not suppress a real specialist context
            // interaction or teach the client a target through a hidden actor lookup.
            let dest = {
                let host = app.world().resource::<SimHost>();
                let grid = host.grid().unwrap();
                (6..30)
                    .flat_map(|y| (4..45).map(move |x| CellPos::new(x, y)))
                    .find(|cell| {
                        grid.cell(*cell)
                            .is_some_and(|c| terrain_enterable(c, domain))
                            && grid.building(*cell).is_none()
                            && [sim::Domain::Land, sim::Domain::Air]
                                .into_iter()
                                .all(|layer| {
                                    grid.holders_for(*cell, layer).iter().all(Option::is_none)
                                })
                            && host
                                .props()
                                .is_none_or(|props| props.id_at(*cell).is_none())
                    })
                    .expect("genuinely empty legal destination in the controlled48x48 fixture")
            };
            assert!(
                crate::ra7_controls::known_at(app.world().resource::<SimHost>(), Some(dest))
                    .is_none(),
                "{section}: empty fixture must offer no contextual actor"
            );
            c62_hover(&mut app, window, dest);
            assert_eq!(
                app.world().resource::<OrderPreview>().intent,
                OrderIntent::Move,
                "{section}: real preview"
            );
            click(&mut app, MouseButton::Right);
            assert_eq!(
                c62_pending(&app),
                vec![Command::Move {
                    units: vec![*id],
                    target: dest
                }],
                "{section}: actual sink"
            );
            let before_tick = app.world().resource::<SimHost>().tick();
            crate::sim_host::tests::run_sink_tick_for_test(app.world_mut());
            assert_eq!(app.world().resource::<SimHost>().tick(), before_tick + 1);
            let events = app
                .world()
                .resource::<bevy::ecs::message::Messages<sim::SimEvent>>();
            assert!(
                !events
                    .iter_current_update_messages()
                    .any(|e| matches!(e, sim::SimEvent::CommandRejected { .. })),
                "{section}: native command acceptance"
            );
            assert!(c62_pending(&app).is_empty());
            app.world_mut().resource_scope(|_, mut host: Mut<SimHost>| {
                crate::sim_host::tests::assert_native_cells_for_test(&mut host);
            });
        }
    }

    fn c62_wall_inputs(name: &str) {
        use bevy::ecs::system::RunSystemOnce;
        let (host, wall, normal, factory, eligible) =
            crate::minimap::ra9_behavior87_routes::c62_wall(name);
        assert!(!own_producers(&host).contains(&wall));
        assert!(!own_yards(&host).contains(&wall));
        assert!(!own_garrisons(&host).contains(&wall));
        assert_eq!(own_building(&host, Some(CellPos::new(18, 14))), Some(wall));
        let (mut app, window) = c62_input(host, None, normal);
        app.world_mut().resource_mut::<HoveredCell>().0 = Some(CellPos::new(18, 14));
        cursor(&mut app, window, Some(Vec2::new(640.0, 400.0)));
        click(&mut app, MouseButton::Left);
        assert!(app.world().resource::<Selection>().0.is_empty());
        assert_eq!(app.world().resource::<SelectedBuilding>().0, None);
        click(&mut app, MouseButton::Left);
        assert_eq!(app.world().resource::<SelectedBuilding>().0, None);
        // Ordinary rectangle still selects only real UnitViews, never this native Building.
        cursor(&mut app, window, Some(Vec2::new(600.0, 350.0)));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        cursor(&mut app, window, Some(Vec2::new(880.0, 450.0)));
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
        assert_eq!(
            app.world().resource::<Selection>().0,
            BTreeSet::from([normal])
        );
        assert!(!app.world().resource::<Selection>().0.contains(&wall));
        app.world_mut().resource_mut::<HoveredCell>().0 = Some(CellPos::new(18, 14));
        cursor(&mut app, window, Some(Vec2::new(640.0, 400.0)));
        click(&mut app, MouseButton::Left);
        c62_hover(&mut app, window, CellPos::new(20, 14));
        assert_eq!(
            app.world().resource::<OrderPreview>().intent,
            OrderIntent::None
        );
        click(&mut app, MouseButton::Right);
        assert!(c62_pending(&app).is_empty());
        assert_eq!(app.world().resource::<CommandSink>().accepted(), 0);
        // Real pruning agrees with the ordinary-click building policy.
        app.world_mut().resource_mut::<SelectedBuilding>().0 = Some(wall);
        app.world_mut()
            .run_system_once(prune_selected_building)
            .unwrap();
        assert_eq!(app.world().resource::<SelectedBuilding>().0, None);
        c62_cycle_from_normal(&mut app, window, normal, wall, &eligible);
        c62_key(&mut app, KeyCode::KeyT);
        assert_eq!(
            app.world().resource::<Selection>().0,
            BTreeSet::from([normal])
        );
        c62_key(&mut app, KeyCode::KeyP);
        assert_eq!(
            app.world().resource::<Selection>().0,
            eligible
                .iter()
                .copied()
                .filter(|id| *id != normal)
                .collect()
        );
        // Positive selectable factory: its native Producer, not a generic Selectable flag,
        // supplies the actual Rally preview and exact sink command.
        app.world_mut().resource_mut::<HoveredCell>().0 = Some(CellPos::new(14, 14));
        cursor(&mut app, window, Some(Vec2::new(300.0, 200.0)));
        click(&mut app, MouseButton::Left);
        assert!(app.world().resource::<Selection>().0.is_empty());
        assert_eq!(app.world().resource::<SelectedBuilding>().0, Some(factory));
        c62_hover(&mut app, window, CellPos::new(20, 14));
        assert_eq!(
            app.world().resource::<OrderPreview>().intent,
            OrderIntent::Rally
        );
        assert_eq!(app.world().resource::<OrderPreview>().target, Some(factory));
        click(&mut app, MouseButton::Right);
        assert_eq!(
            c62_pending(&app),
            vec![Command::SetRallyPoint {
                building: factory,
                cell: CellPos::new(20, 14)
            }]
        );
        crate::sim_host::tests::run_sink_tick_for_test(app.world_mut());
        {
            let host = app.world().resource::<SimHost>();
            let mut q = host
                .world()
                .try_query::<(&SimId, &sim::Producer)>()
                .unwrap();
            assert_eq!(
                q.iter(host.world())
                    .find(|(id, _)| **id == factory)
                    .unwrap()
                    .1
                    .rally,
                Some(CellPos::new(20, 14))
            );
        }
        c62_select_normal(&mut app, window);
        // Cycle from the Engineer to the actual Grizzly before the friendly-wall order.
        // Engineer interaction_label legitimately offers REPAIR even for a full own building;
        // this control must not invent an absent Engineer interaction.
        let mover = *eligible.iter().find(|id| **id != normal).unwrap();
        c62_key(&mut app, KeyCode::KeyN);
        assert_eq!(
            app.world().resource::<Selection>().0,
            BTreeSet::from([mover])
        );
        c62_hover(&mut app, window, CellPos::new(18, 14));
        assert_eq!(
            app.world().resource::<OrderPreview>().intent,
            OrderIntent::Friendly
        );
        assert_eq!(app.world().resource::<OrderPreview>().target, Some(wall));
        click(&mut app, MouseButton::Right);
        assert_eq!(
            c62_pending(&app),
            vec![Command::Move {
                units: vec![mover],
                target: CellPos::new(18, 14)
            }]
        );
        crate::sim_host::tests::run_sink_tick_for_test(app.world_mut());
        let host = app.world().resource::<SimHost>();
        assert_eq!(
            host.grid().unwrap().building(CellPos::new(18, 14)),
            Some(wall)
        );
        let mut q = host.world().try_query::<(&SimId, &sim::Mobile)>().unwrap();
        assert_ne!(
            q.iter(host.world())
                .find(|(id, _)| **id == mover)
                .unwrap()
                .1
                .cell,
            CellPos::new(18, 14)
        );
        assert!(
            !app.world()
                .resource::<bevy::ecs::message::Messages<sim::SimEvent>>()
                .iter_current_update_messages()
                .any(|e| matches!(e, sim::SimEvent::CommandRejected { .. }))
        );
        // No UnitView is forged for a Building. Walls remain valid friendly coordinate targets;
        // absence from SelectedBuilding is not a blanket prohibition on legal unit orders.
    }
    #[test]
    fn ra9_c62_allied_wall_actual_selection_cycle_preview_sink() {
        c62_wall_inputs("fortress_walls");
    }
    #[test]
    fn ra9_c62_soviet_wall_actual_selection_cycle_preview_sink() {
        c62_wall_inputs("soviet_walls");
    }
    #[test]
    fn ra9_c62_bridge_hut_current_narrow_selection_cycle_preview_sink() {
        c62_wall_inputs("bridge_hut");
    }
}
