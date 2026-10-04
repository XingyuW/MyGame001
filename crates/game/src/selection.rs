//! Selection and orders (PLAN.md section 7, M2): selecting the player's units by click, by
//! dragging a box, and additively with Shift, and ordering the selection to move (right click on
//! a cell) or to stop (S).
//!
//! The interface changes the simulation only by pushing commands into `CommandQueue`
//! (invariant I5). The decisions are pure functions, unit tested below; the systems only read
//! input and apply them.

use std::collections::BTreeSet;

use bevy::camera::CameraUpdateSystems;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use bevy::window::PrimaryWindow;
use sim::{CellPos, Command, CommandQueue, IssuedCommand, SimId};

use crate::map::{HoveredCell, MapView};
use crate::projection::cell_corners;
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
            .add_systems(
                PostUpdate,
                (prune_selection, select_units, issue_orders, draw_selection)
                    .chain()
                    .after(TransformSystems::Propagate)
                    .after(CameraUpdateSystems),
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

/// Units that no longer exist leave the selection.
fn prune_selection(registry: Res<UnitViews>, mut selection: ResMut<Selection>) {
    selection.0.retain(|id| registry.0.contains_key(id));
}

fn select_units(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform)>,
    mut drag: ResMut<DragBox>,
    mut selection: ResMut<Selection>,
    views: Query<(&UnitView, &Transform)>,
) {
    let (camera, camera_transform) = *camera;
    if buttons.just_pressed(MouseButton::Left) {
        drag.start = window.cursor_position();
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

fn issue_orders(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    hovered: Res<HoveredCell>,
    selection: Res<Selection>,
    mut queue: ResMut<CommandQueue>,
) {
    let mut commands = Vec::new();
    if buttons.just_pressed(MouseButton::Right) {
        commands.extend(move_command(&selection.0, hovered.0));
    }
    if keys.just_pressed(KeyCode::KeyS) {
        commands.extend(stop_command(&selection.0));
    }
    for command in commands {
        queue.push(IssuedCommand {
            player: HUMAN,
            command,
        });
    }
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
