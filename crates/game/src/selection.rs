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
//! (`SetRallyPoint`), where every unit it produces from then on goes. `R` and the sidebar's
//! Rally button still set the rally point of every producer at once (`crate::construction`).
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

/// The player's selected unit-producing building, if any: the one whose rally point a right
/// click sets. Selecting units deselects it, and it deselects the units.
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
}

impl OrderIntent {
    /// The colour of the cursor marker and label.
    pub fn colour(self) -> Color {
        match self {
            Self::None => Color::NONE,
            Self::Move => Color::srgb(0.35, 1.0, 0.4),
            Self::Attack => Color::srgb(1.0, 0.12, 0.08),
            Self::Harvest => Color::srgb(1.0, 0.82, 0.25),
            Self::Friendly => Color::srgb(0.35, 0.8, 1.0),
            Self::Invalid => Color::srgb(0.62, 0.62, 0.62),
            Self::Rally => RALLY_COLOUR,
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

/// The player's own units and their ground points in the world.
fn own_units(views: &Query<(&UnitView, &Transform)>) -> Vec<(SimId, Vec2)> {
    views
        .iter()
        .filter(|(view, _)| view.owner == HUMAN)
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

/// Units that no longer exist leave the selection and the control groups.
pub(crate) fn prune_selection(
    registry: Res<UnitViews>,
    mut selection: ResMut<Selection>,
    mut groups: ResMut<ControlGroups>,
) {
    selection.0.retain(|id| registry.0.contains_key(id));
    groups.prune(|id| registry.0.contains_key(&id));
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
    if !selection.0.is_empty() || !own_producers(&host).contains(&id) {
        building.0 = None;
    }
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
    if !window.focused || keys.pressed(KeyCode::Space) || buttons.pressed(MouseButton::Middle) {
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
    let units = own_units(&views);
    let mut producer = None;
    let picked = if start.distance(end) < DRAG_THRESHOLD_PX {
        let pick = unit_at(end_world, &units);
        // No unit under the cursor: a click on a factory selects the factory.
        producer = click_pick(
            pick,
            own_building(&host, hovered.0).filter(|id| own_producers(&host).contains(id)),
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
fn shown_enemy_units(views: &Query<(&UnitView, &Transform, &Visibility)>) -> Vec<(SimId, Vec2)> {
    views
        .iter()
        .filter(|(view, _, visibility)| view.owner != HUMAN && **visibility != Visibility::Hidden)
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
    let id = host.grid()?.building(cell?)?;
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

/// The scenery standing in `cell` (`sim::Props`: trees, houses, city blocks) that the human
/// player has explored: a right click with armed units selected orders them to destroy it.
fn shown_scenery(host: &SimHost, cell: Option<CellPos>) -> Option<SimId> {
    let cell = cell?;
    host.players()?
        .get(HUMAN)?
        .vision()
        .is_explored(cell)
        .then(|| host.props()?.id_at(cell))?
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
    let id = host.grid()?.building(cell?)?;
    let mut buildings = host.world().try_query::<(&SimId, &Owner)>()?;
    buildings
        .iter(host.world())
        .find(|(other, owner)| **other == id && owner.0 == HUMAN)
        .map(|(id, _)| *id)
}

/// Works out what a right click would order ([`order_intent`]) before it happens.
#[allow(clippy::too_many_arguments)]
pub(crate) fn preview_order(
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
        producer: selection.0.is_empty() && building.0.is_some(),
        cell: hovered.0,
        ..default()
    };
    if over_map && !selection.0.is_empty() {
        let building = shown_enemy_building(&host, hovered.0);
        facts.enemy = point
            .and_then(|point| enemy_at(point, &shown_enemy_units(&views), building))
            .or(building);
        let own: Vec<(SimId, Vec2)> = views
            .iter()
            .filter(|(view, ..)| view.owner == HUMAN)
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
    let intent = order_intent(facts);
    let next = OrderPreview {
        intent,
        target: match intent {
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
            OrderIntent::Attack => SystemCursorIcon::Crosshair,
            OrderIntent::Invalid => SystemCursorIcon::NotAllowed,
            OrderIntent::Friendly | OrderIntent::Harvest | OrderIntent::Rally => {
                SystemCursorIcon::Pointer
            }
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
            .and_then(|point| enemy_at(point, &shown_enemy_units(&views), building))
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
    for (view, transform) in &views {
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
        OrderIntent::Attack => {
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
        OrderIntent::Friendly => {
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
        OrderIntent::Move | OrderIntent::Harvest => {
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
}
