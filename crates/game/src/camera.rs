//! The 2D camera: panning by arrow keys, screen edge, and middle mouse drag, zoom by mouse wheel
//! within limits, and clamping to the map (PLAN.md section 7, M1; docs/DECISIONS.md, D10). Since
//! M7 the options (`crate::options::CameraOptions`, D52) switch edge scrolling off and scale the
//! pan speed.
//!
//! The arithmetic lives in pure functions so that it can be unit tested; the systems only read
//! input and apply the results.

use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::options::CameraOptions;

/// Keyboard and edge panning speed in world units per second at zoom scale 1. At other scales
/// the speed is multiplied by the scale, so the map moves across the screen at the same rate.
pub const PAN_SPEED: f32 = 1000.0;

/// Width in logical pixels of the band along each window edge that pans the camera.
pub const EDGE_PAN_MARGIN: f32 = 8.0;

/// Smallest projection scale (most zoomed in): one world unit covers two screen pixels.
pub const MIN_SCALE: f32 = 0.5;

/// Largest projection scale (most zoomed out).
pub const MAX_SCALE: f32 = 2.0;

/// Factor by which one scroll line changes the projection scale.
pub const ZOOM_PER_LINE: f32 = 1.1;

/// Scroll distance in pixels treated as one line (touchpads report pixels).
pub const PIXELS_PER_LINE: f32 = 20.0;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraBounds>()
            .init_resource::<CameraOptions>()
            .init_resource::<MiddleDrag>()
            .add_systems(PostStartup, spawn_camera)
            .add_systems(
                Update,
                (pan_camera, drag_camera, zoom_camera, clamp_camera).chain(),
            );
    }
}

/// World rectangle the view must stay inside; `None` until a map is loaded.
#[derive(Resource, Debug, Default)]
pub struct CameraBounds(pub Option<Rect>);

/// Cursor position of the previous frame while the middle mouse button is held.
#[derive(Resource, Debug, Default)]
struct MiddleDrag {
    last_cursor: Option<Vec2>,
}

// --- Pure functions -------------------------------------------------------------------------

/// Unit (or zero) world direction for the pressed arrow keys.
pub fn keyboard_direction(left: bool, right: bool, up: bool, down: bool) -> Vec2 {
    let axis = |negative: bool, positive: bool| f32::from(positive) - f32::from(negative);
    Vec2::new(axis(left, right), axis(down, up)).normalize_or_zero()
}

/// Unit (or zero) world direction for a cursor at `cursor` (logical pixels, origin at the top
/// left of the window, y down) in a window of size `window`.
pub fn edge_direction(cursor: Vec2, window: Vec2, margin: f32) -> Vec2 {
    let axis = |position: f32, extent: f32| {
        if position < margin {
            -1.0
        } else if position >= extent - margin {
            1.0
        } else {
            0.0
        }
    };
    // Screen y grows downwards, world y upwards.
    Vec2::new(axis(cursor.x, window.x), -axis(cursor.y, window.y)).normalize_or_zero()
}

/// Camera translation that keeps the world point under the cursor fixed while the cursor
/// moves from `previous` to `current` (logical pixels, y down) at projection scale `scale`.
pub fn drag_translation(previous: Vec2, current: Vec2, scale: f32) -> Vec2 {
    Vec2::new(previous.x - current.x, current.y - previous.y) * scale
}

/// Projection scale after scrolling `lines` lines (positive zooms in), within the limits.
pub fn zoomed_scale(scale: f32, lines: f32) -> f32 {
    (scale * ZOOM_PER_LINE.powf(-lines)).clamp(MIN_SCALE, MAX_SCALE)
}

/// Camera centre that keeps a view of half size `half_view` inside `bounds`. Along an axis on
/// which the view is larger than the bounds, the view is centred on the bounds instead.
pub fn clamp_centre(centre: Vec2, half_view: Vec2, bounds: Rect) -> Vec2 {
    let axis = |centre: f32, half: f32, min: f32, max: f32| {
        if max - min <= 2.0 * half {
            (min + max) / 2.0
        } else {
            centre.clamp(min + half, max - half)
        }
    };
    Vec2::new(
        axis(centre.x, half_view.x, bounds.min.x, bounds.max.x),
        axis(centre.y, half_view.y, bounds.min.y, bounds.max.y),
    )
}

/// Camera centre that keeps the part of the view beside the sidebar inside `bounds` (M5): the
/// view of `viewport` logical pixels at projection `scale` has its right `inset` pixels covered
/// by the sidebar, so the visible part is narrower and its centre lies left of the camera's
/// ([`clamp_centre`] applied to the visible part). With `inset` 0 this is [`clamp_centre`].
pub fn clamp_centre_beside(
    centre: Vec2,
    viewport: Vec2,
    scale: f32,
    inset: f32,
    bounds: Rect,
) -> Vec2 {
    let inset = inset.clamp(0.0, viewport.x);
    let shift = Vec2::new(inset * scale / 2.0, 0.0);
    let visible = Vec2::new(viewport.x - inset, viewport.y) * scale / 2.0;
    clamp_centre(centre - shift, visible, bounds) + shift
}

// --- Systems --------------------------------------------------------------------------------

fn spawn_camera(mut commands: Commands, bounds: Res<CameraBounds>) {
    let centre = bounds.0.map_or(Vec2::ZERO, |rect| rect.center());
    commands.spawn((Camera2d, Transform::from_translation(centre.extend(0.0))));
}

fn orthographic_scale(projection: &Projection) -> Option<f32> {
    match projection {
        Projection::Orthographic(ortho) => Some(ortho.scale),
        _ => None,
    }
}

pub(crate) fn pan_camera(
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window, With<PrimaryWindow>>,
    time: Res<Time>,
    options: Res<CameraOptions>,
    camera: Single<(&mut Transform, &Projection), With<Camera2d>>,
) {
    let (mut transform, projection) = camera.into_inner();
    let Some(scale) = orthographic_scale(projection) else {
        return;
    };
    let mut direction = keyboard_direction(
        keys.pressed(KeyCode::ArrowLeft),
        keys.pressed(KeyCode::ArrowRight),
        keys.pressed(KeyCode::ArrowUp),
        keys.pressed(KeyCode::ArrowDown),
    );
    if options.edge_scrolling
        && window.focused
        && let Some(cursor) = window.cursor_position()
    {
        direction = (direction + edge_direction(cursor, window.size(), EDGE_PAN_MARGIN))
            .normalize_or_zero();
    }
    let step = direction * PAN_SPEED * options.speed_factor() * scale * time.delta_secs();
    transform.translation += step.extend(0.0);
}

fn drag_camera(
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut drag: ResMut<MiddleDrag>,
    camera: Single<(&mut Transform, &Projection), With<Camera2d>>,
) {
    let cursor = window.cursor_position();
    if !buttons.pressed(MouseButton::Middle) || cursor.is_none() {
        drag.last_cursor = None;
        return;
    }
    let (mut transform, projection) = camera.into_inner();
    if let (Some(previous), Some(current), Some(scale)) =
        (drag.last_cursor, cursor, orthographic_scale(projection))
    {
        transform.translation += drag_translation(previous, current, scale).extend(0.0);
    }
    drag.last_cursor = cursor;
}

pub(crate) fn zoom_camera(
    scroll: Res<AccumulatedMouseScroll>,
    mut projection: Single<&mut Projection, With<Camera2d>>,
) {
    if scroll.delta.y == 0.0 {
        return;
    }
    let lines = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / PIXELS_PER_LINE,
    };
    if let Projection::Orthographic(ortho) = projection.as_mut() {
        ortho.scale = zoomed_scale(ortho.scale, lines);
    }
}

pub(crate) fn clamp_camera(
    bounds: Res<CameraBounds>,
    camera: Single<(&Camera, &Projection, &mut Transform)>,
) {
    let (camera, projection, mut transform) = camera.into_inner();
    let (Some(bounds), Some(viewport), Some(scale)) = (
        bounds.0,
        camera.logical_viewport_size(),
        orthographic_scale(projection),
    ) else {
        return;
    };
    // With `ScalingMode::WindowSize` (the default) the view spans the logical viewport size
    // times the projection scale in world units; the sidebar covers its right part (M5).
    let centre = clamp_centre_beside(
        transform.translation.truncate(),
        viewport,
        scale,
        crate::sidebar::SIDEBAR_WIDTH,
        bounds,
    );
    transform.translation.x = centre.x;
    transform.translation.y = centre.y;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 4096 × 2048 rectangle, the bounds of the 64×64 demo map with 64×32 pixel tiles.
    fn bounds() -> Rect {
        Rect::new(-2048.0, -2048.0, 2048.0, 0.0)
    }

    const HALF_VIEW: Vec2 = Vec2::new(640.0, 360.0);

    #[test]
    fn centre_inside_the_bounds_is_unchanged() {
        let centre = Vec2::new(100.0, -1000.0);
        assert_eq!(clamp_centre(centre, HALF_VIEW, bounds()), centre);
    }

    #[test]
    fn clamping_at_all_four_borders() {
        let b = bounds();
        let clamp = |x: f32, y: f32| clamp_centre(Vec2::new(x, y), HALF_VIEW, b);
        // Left, right, top, bottom: the view edge lands exactly on the border.
        assert_eq!(
            clamp(-5000.0, -1000.0),
            Vec2::new(b.min.x + HALF_VIEW.x, -1000.0)
        );
        assert_eq!(
            clamp(5000.0, -1000.0),
            Vec2::new(b.max.x - HALF_VIEW.x, -1000.0)
        );
        assert_eq!(clamp(0.0, 5000.0), Vec2::new(0.0, b.max.y - HALF_VIEW.y));
        assert_eq!(clamp(0.0, -5000.0), Vec2::new(0.0, b.min.y + HALF_VIEW.y));
        // Corners clamp both axes.
        assert_eq!(
            clamp(-5000.0, 5000.0),
            Vec2::new(b.min.x + HALF_VIEW.x, b.max.y - HALF_VIEW.y)
        );
        assert_eq!(
            clamp(5000.0, -5000.0),
            Vec2::new(b.max.x - HALF_VIEW.x, b.min.y + HALF_VIEW.y)
        );
        // Exactly at the limit is unchanged.
        let limit = Vec2::new(b.max.x - HALF_VIEW.x, b.min.y + HALF_VIEW.y);
        assert_eq!(clamp(limit.x, limit.y), limit);
    }

    #[test]
    fn view_larger_than_the_bounds_is_centred_on_that_axis() {
        let b = bounds();
        // Taller than the bounds (2048) but narrower (4096): centred vertically only.
        let half = Vec2::new(1000.0, 1100.0);
        assert_eq!(
            clamp_centre(Vec2::new(-5000.0, 900.0), half, b),
            Vec2::new(b.min.x + half.x, b.center().y)
        );
        // Larger on both axes, and exactly equal on one: centred on both.
        for half in [Vec2::new(3000.0, 3000.0), Vec2::new(2048.0, 1024.0)] {
            assert_eq!(clamp_centre(Vec2::new(77.0, -5.0), half, b), b.center());
        }
    }

    #[test]
    fn zoom_stays_within_its_limits() {
        assert_eq!(zoomed_scale(1.0, 0.0), 1.0);
        assert!((zoomed_scale(1.0, 1.0) - 1.0 / ZOOM_PER_LINE).abs() < 1e-6);
        assert!((zoomed_scale(1.0, -1.0) - ZOOM_PER_LINE).abs() < 1e-6);
        assert_eq!(zoomed_scale(1.0, 1000.0), MIN_SCALE);
        assert_eq!(zoomed_scale(1.0, -1000.0), MAX_SCALE);
        assert_eq!(zoomed_scale(MIN_SCALE, 1.0), MIN_SCALE);
        assert_eq!(zoomed_scale(MAX_SCALE, -1.0), MAX_SCALE);
        // Many small steps never leave the limits either.
        let mut scale = 1.0;
        for _ in 0..200 {
            scale = zoomed_scale(scale, -0.25);
            assert!((MIN_SCALE..=MAX_SCALE).contains(&scale));
        }
        assert_eq!(scale, MAX_SCALE);
    }

    #[test]
    fn keyboard_directions() {
        assert_eq!(keyboard_direction(false, false, false, false), Vec2::ZERO);
        assert_eq!(keyboard_direction(true, false, false, false), Vec2::NEG_X);
        assert_eq!(keyboard_direction(false, true, false, false), Vec2::X);
        assert_eq!(keyboard_direction(false, false, true, false), Vec2::Y);
        assert_eq!(keyboard_direction(false, false, false, true), Vec2::NEG_Y);
        // Opposite keys cancel; diagonals are not faster.
        assert_eq!(keyboard_direction(true, true, false, false), Vec2::ZERO);
        let diagonal = keyboard_direction(false, true, true, false);
        assert!((diagonal.length() - 1.0).abs() < 1e-6);
        assert!(diagonal.x > 0.0 && diagonal.y > 0.0);
    }

    #[test]
    fn edge_directions() {
        let window = Vec2::new(1280.0, 720.0);
        let edge = |x: f32, y: f32| edge_direction(Vec2::new(x, y), window, EDGE_PAN_MARGIN);
        assert_eq!(edge(640.0, 360.0), Vec2::ZERO);
        assert_eq!(edge(0.0, 360.0), Vec2::NEG_X);
        assert_eq!(edge(1279.0, 360.0), Vec2::X);
        assert_eq!(edge(640.0, 0.0), Vec2::Y, "top of the window pans up");
        assert_eq!(edge(640.0, 719.0), Vec2::NEG_Y, "bottom pans down");
        // The band is EDGE_PAN_MARGIN pixels wide on each side.
        assert_eq!(edge(EDGE_PAN_MARGIN - 0.5, 360.0), Vec2::NEG_X);
        assert_eq!(edge(EDGE_PAN_MARGIN, 360.0), Vec2::ZERO);
        assert_eq!(edge(window.x - EDGE_PAN_MARGIN - 0.5, 360.0), Vec2::ZERO);
        assert_eq!(edge(window.x - EDGE_PAN_MARGIN, 360.0), Vec2::X);
        // Corners pan diagonally at unit speed.
        let corner = edge(0.0, 0.0);
        assert!((corner.length() - 1.0).abs() < 1e-6);
        assert!(corner.x < 0.0 && corner.y > 0.0);
    }

    #[test]
    fn drag_keeps_the_grabbed_world_point_under_the_cursor() {
        let window = Vec2::new(1280.0, 720.0);
        // World point under a cursor for a camera at `centre` (ScalingMode::WindowSize).
        let world_under = |centre: Vec2, cursor: Vec2, scale: f32| {
            centre + Vec2::new(cursor.x - window.x / 2.0, window.y / 2.0 - cursor.y) * scale
        };
        for scale in [MIN_SCALE, 1.0, MAX_SCALE] {
            let centre = Vec2::new(300.0, -800.0);
            let previous = Vec2::new(400.0, 300.0);
            let current = Vec2::new(250.0, 520.0);
            let grabbed = world_under(centre, previous, scale);
            let moved = centre + drag_translation(previous, current, scale);
            assert_eq!(world_under(moved, current, scale), grabbed, "scale {scale}");
        }
    }

    #[test]
    fn the_part_of_the_view_beside_the_sidebar_stays_inside_the_bounds() {
        let b = bounds();
        let viewport = HALF_VIEW * 2.0;
        // Without an inset this is `clamp_centre`.
        for point in [
            Vec2::new(-5000.0, 0.0),
            Vec2::new(5000.0, 5000.0),
            Vec2::ZERO,
        ] {
            assert_eq!(
                clamp_centre_beside(point, viewport, 1.0, 0.0, b),
                clamp_centre(point, HALF_VIEW, b)
            );
        }
        // With the sidebar covering 240 px at scale 2, the visible part's right edge reaches the
        // right border, and its left edge the left border.
        let (inset, scale) = (240.0, 2.0);
        let right = clamp_centre_beside(Vec2::new(5000.0, 0.0), viewport, scale, inset, b);
        assert_eq!(right.x + viewport.x * scale / 2.0 - inset * scale, b.max.x);
        let left = clamp_centre_beside(Vec2::new(-5000.0, 0.0), viewport, scale, inset, b);
        assert_eq!(left.x - viewport.x * scale / 2.0, b.min.x);
        // An inset wider than the view counts as the whole view: the visible part has width 0
        // and is clamped like a point.
        let all = clamp_centre_beside(Vec2::new(5000.0, 0.0), viewport, 1.0, 1.0e6, b);
        assert_eq!(all.x - viewport.x / 2.0, b.max.x);
    }
}
