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
//! The interface changes the simulation only by pushing commands into `CommandQueue`, through
//! `crate::sim_host::CommandSink` (invariant I5), and reads it through `SimHost` (D34). The
//! decisions are pure functions, unit tested below; the systems only read input and apply them.

use std::collections::BTreeSet;

use bevy::camera::CameraUpdateSystems;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use bevy::window::PrimaryWindow;
use sim::{Building, CellPos, Command, Harvester, Owner, SimId, Weapon};

use crate::construction::InterfaceMode;
use crate::hotkeys::{ControlGroups, Hotkey, pressed_hotkeys};
use crate::map::{HoveredCell, MapTerrain, MapView};
use crate::projection::cell_corners;
use crate::sidebar::over_sidebar;
use crate::sim_host::{CommandSink, SimHost};
use crate::units::{GROUND_OFFSET_PX, HUMAN, UnitView, UnitViews};

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
            .init_resource::<DragBox>()
            .init_resource::<ControlGroups>()
            .add_systems(
                PostUpdate,
                (
                    prune_selection,
                    control_groups,
                    select_units,
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

/// The cursor position (logical pixels) where the left button went down, while it is held.
#[derive(Resource, Debug, Default)]
pub struct DragBox {
    pub start: Option<Vec2>,
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
    views: Query<(&UnitView, &Transform)>,
) {
    let (camera, camera_transform) = *camera;
    if buttons.just_pressed(MouseButton::Left) {
        // A press over the sidebar, or in a mode whose clicks place, sell, or set a rally
        // point, does not select.
        drag.start = window
            .cursor_position()
            .filter(|&cursor| !over_sidebar(cursor, window.width()))
            .filter(|_| *mode == InterfaceMode::Normal);
    }
    if !buttons.just_released(MouseButton::Left) {
        return;
    }
    let Some(start) = drag.start.take() else {
        return;
    };
    let end = window.cursor_position().unwrap_or(start);
    let to_world = |screen: Vec2| camera.viewport_to_world_2d(camera_transform, screen).ok();
    let (Some(start_world), Some(end_world)) = (to_world(start), to_world(end)) else {
        return;
    };
    let units = own_units(&views);
    let picked = if start.distance(end) < DRAG_THRESHOLD_PX {
        unit_at(end_world, &units).into_iter().collect()
    } else {
        units_in_box(start_world, end_world, &units)
    };
    let additive = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    selection.0 = apply_selection(&selection.0, picked, additive);
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
) {
    let mut commands = Vec::new();
    // In a mode of `crate::construction`, a right click leaves the mode instead.
    if buttons.just_pressed(MouseButton::Right) && *mode == InterfaceMode::Normal {
        let (camera, camera_transform) = *camera;
        let building = shown_enemy_building(&host, hovered.0);
        let enemy = cursor_world(&window, camera, camera_transform)
            .and_then(|point| enemy_at(point, &shown_enemy_units(&views), building))
            .or(building);
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
            Hotkey::AttackMove => commands.extend(attack_move_command(&selection.0, hovered.0)),
            _ => {}
        }
    }
    sink.issue(commands);
}

fn draw_selection(
    selection: Res<Selection>,
    drag: Res<DragBox>,
    map: Option<Res<MapView>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform)>,
    views: Query<(&UnitView, &Transform)>,
    mut gizmos: Gizmos,
) {
    let Some(map) = map else {
        return;
    };
    // A small diamond under each selected unit, a quarter of a cell from its ground point.
    let marker = cell_corners(CellPos::new(0, 0), map.tile).map(|corner| {
        (corner - crate::projection::cell_centre(CellPos::new(0, 0), map.tile)) * 0.6
    });
    let green = Color::srgb(0.3, 1.0, 0.3);
    for (view, transform) in &views {
        if !selection.0.contains(&view.id) {
            continue;
        }
        let ground = transform.translation.truncate() - Vec2::new(0.0, GROUND_OFFSET_PX);
        let [top, right, bottom, left] = marker.map(|corner| ground + corner);
        gizmos.linestrip_2d([top, right, bottom, left, top], green);
    }
    // The box while the left button is dragged.
    let (camera, camera_transform) = *camera;
    if let (Some(start), Some(end)) = (drag.start, cursor_world(&window, camera, camera_transform))
        && let Ok(start) = camera.viewport_to_world_2d(camera_transform, start)
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

#[cfg(test)]
mod tests {
    use super::*;

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
