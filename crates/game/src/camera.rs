//! The 2D camera: arrow keys, optional edge scrolling and anchored rate panning, wheel zoom
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

/// Depth in logical pixels of the band along each window edge that pans the camera. It is the
/// same on all four edges and deep enough that the cursor never has to be pushed against the
/// macOS frame, whose title bar, Dock trigger, menu bar reveal and resize regions lie at or just
/// beyond the content edge (top and bottom were unreliable with the former 8 pixel band).
pub const EDGE_PAN_MARGIN: f32 = 28.0;

/// Fraction of full speed at the inner border of an edge band; it rises linearly to 1 at the
/// window edge, so speed is predictable and the camera does not lurch on entering the band.
pub const EDGE_PAN_MIN_SPEED: f32 = 0.4;

/// Within this distance of a corner (logical pixels), an edge band also pans along the other
/// axis, so diagonal scrolling does not need the cursor exactly in the corner.
pub const EDGE_CORNER_REACH: f32 = 72.0;

/// When the cursor leaves the window (title bar, Dock, another screen) from within this
/// distance of an edge, the camera keeps panning towards that edge at full speed until the
/// cursor comes back or the window loses focus. A fast flick can skip the band itself.
pub const EDGE_EXIT_REACH: f32 = 160.0;

/// Time constant (seconds) of the easing of the pan velocity: starting and stopping take about
/// a tenth of a second instead of jumping, which hides the one-frame flicker of the cursor
/// crossing the window border.
pub const PAN_EASE_SECONDS: f32 = 0.05;

/// Smallest projection scale (most zoomed in): one world unit covers two screen pixels.
pub const MIN_SCALE: f32 = 0.35;

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
            .init_resource::<EdgePan>()
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

/// The fixed anchor of a middle-button or Space + left-button camera gesture.
#[derive(Resource, Debug, Default)]
struct MiddleDrag {
    anchor: Option<Vec2>,
}

/// Edge scrolling state: the last cursor position inside the window (to keep panning after the
/// cursor leaves through an edge) and the eased pan velocity (unit speed is full speed).
#[derive(Resource, Debug, Default)]
pub(crate) struct EdgePan {
    last_inside: Option<Vec2>,
    velocity: Vec2,
}

// --- Pure functions -------------------------------------------------------------------------

/// Unit (or zero) world direction for the pressed arrow keys.
pub fn keyboard_direction(left: bool, right: bool, up: bool, down: bool) -> Vec2 {
    let axis = |negative: bool, positive: bool| f32::from(positive) - f32::from(negative);
    Vec2::new(axis(left, right), axis(down, up)).normalize_or_zero()
}

/// World pan velocity (length at most 1, 1 being full speed) for a cursor at `cursor` (logical
/// pixels, origin at the top left of the window, y down) in a window of size `window`, with
/// bands `margin` pixels deep along every edge. The speed rises from [`EDGE_PAN_MIN_SPEED`] at
/// the inner border of a band to 1 at the edge; near a corner ([`EDGE_CORNER_REACH`]) the
/// perpendicular axis joins in, and diagonals are no faster than straight lines.
pub fn edge_direction(cursor: Vec2, window: Vec2, margin: f32) -> Vec2 {
    let margin = margin.max(1.0);
    // Signed strength along one axis: negative towards the low edge, positive towards the high.
    let axis = |position: f32, extent: f32| {
        let (low, high) = (position, extent - position);
        let ramp = |distance: f32| {
            EDGE_PAN_MIN_SPEED
                + (1.0 - EDGE_PAN_MIN_SPEED) * (1.0 - distance / margin).clamp(0.0, 1.0)
        };
        if low < margin {
            -ramp(low.max(0.0))
        } else if high <= margin {
            ramp(high.max(0.0))
        } else {
            0.0
        }
    };
    // The side of an axis within corner reach, if any.
    let corner = |position: f32, extent: f32| {
        let reach = EDGE_CORNER_REACH.max(margin);
        if position < reach {
            -1.0
        } else if extent - position <= reach {
            1.0
        } else {
            0.0
        }
    };
    let (mut x, mut y) = (axis(cursor.x, window.x), axis(cursor.y, window.y));
    if x != 0.0 && y == 0.0 {
        y = corner(cursor.y, window.y) * x.abs();
    } else if y != 0.0 && x == 0.0 {
        x = corner(cursor.x, window.x) * y.abs();
    }
    // Screen y grows downwards, world y upwards.
    let velocity = Vec2::new(x, -y);
    if velocity.length() > 1.0 {
        velocity.normalize()
    } else {
        velocity
    }
}

/// Full-speed world direction for a cursor that has left the window, last seen at `last`: towards
/// the edges it was within [`EDGE_EXIT_REACH`] of (the edge it left through), or zero if it left
/// from the middle of the window (which only happens when it jumps, e.g. on a focus change).
pub fn exit_direction(last: Vec2, window: Vec2) -> Vec2 {
    let reach = EDGE_EXIT_REACH.min(window.min_element() / 2.0).max(1.0);
    let side = |position: f32, extent: f32| {
        let (low, high) = (position, extent - position);
        if low < reach && low <= high {
            -1.0
        } else if high < reach {
            1.0
        } else {
            0.0
        }
    };
    let (x, y) = (side(last.x, window.x), side(last.y, window.y));
    // Only the nearer edge counts unless the cursor left close to a corner.
    let (dx, dy) = (last.x.min(window.x - last.x), last.y.min(window.y - last.y));
    let (x, y) = if x != 0.0 && y != 0.0 && (dx - dy).abs() > EDGE_CORNER_REACH {
        if dx < dy { (x, 0.0) } else { (0.0, y) }
    } else {
        (x, y)
    };
    Vec2::new(x, -y).normalize_or_zero()
}

/// Eases `velocity` towards `target` over `dt` seconds ([`PAN_EASE_SECONDS`]).
pub fn eased_velocity(velocity: Vec2, target: Vec2, dt: f32) -> Vec2 {
    let blend = 1.0 - (-dt.max(0.0) / PAN_EASE_SECONDS).exp();
    let next = velocity.lerp(target, blend);
    // Snap the tail so the camera comes to a definite stop.
    if target == Vec2::ZERO && next.length() < 0.01 {
        Vec2::ZERO
    } else {
        next
    }
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn pan_camera(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window, With<PrimaryWindow>>,
    time: Res<Time>,
    options: Res<CameraOptions>,
    mut edge: ResMut<EdgePan>,
    interface: Query<&Interaction>,
    camera: Single<(&mut Transform, &Projection), With<Camera2d>>,
) {
    let (mut transform, projection) = camera.into_inner();
    let Some(scale) = orthographic_scale(projection) else {
        return;
    };
    if !window.focused {
        *edge = EdgePan::default();
        return;
    }
    let keyboard = keyboard_direction(
        keys.pressed(KeyCode::ArrowLeft),
        keys.pressed(KeyCode::ArrowRight),
        keys.pressed(KeyCode::ArrowUp),
        keys.pressed(KeyCode::ArrowDown),
    );
    let cursor = window.cursor_position();
    // The bands are deep enough to reach interface buttons along the edges (sidebar cards, the
    // strategic buttons): hovering a button never scrolls the map.
    let over_button = interface
        .iter()
        .any(|interaction| *interaction != Interaction::None);
    let mut pointer = Vec2::ZERO;
    if options.edge_scrolling
        && !over_button
        && !buttons.any_pressed([MouseButton::Left, MouseButton::Middle])
    {
        pointer = match cursor {
            Some(cursor) => edge_direction(cursor, window.size(), EDGE_PAN_MARGIN),
            // Outside the window (in the title bar, over the Dock, past a screen edge): keep
            // panning towards the edge the cursor left through.
            None => edge
                .last_inside
                .map_or(Vec2::ZERO, |last| exit_direction(last, window.size())),
        };
    }
    if cursor.is_some() {
        edge.last_inside = cursor;
    }
    let target = (keyboard + pointer).clamp_length_max(1.0);
    let dt = time.delta_secs().min(0.05);
    edge.velocity = eased_velocity(edge.velocity, target, dt);
    let step = edge.velocity * PAN_SPEED * options.speed_factor() * scale * dt;
    transform.translation += step.extend(0.0);
}

/// Rate scrolling: displace a held middle button (or Space + left) from its anchor.
/// Holding still keeps scrolling, so the cursor never needs to reach the OS window border.
pub fn pan_velocity(anchor: Vec2, cursor: Vec2) -> Vec2 {
    let offset = drag_translation(anchor, cursor, 1.0) * -1.0;
    let distance = offset.length();
    offset.normalize_or_zero() * ((distance - 4.0) / 76.0).clamp(0.0, 1.0)
}
fn drag_camera(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window, With<PrimaryWindow>>,
    time: Res<Time>,
    options: Res<CameraOptions>,
    mut drag: ResMut<MiddleDrag>,
    camera: Single<(&mut Transform, &Projection), With<Camera2d>>,
) {
    let held = buttons.pressed(MouseButton::Middle)
        || (keys.pressed(KeyCode::Space) && buttons.pressed(MouseButton::Left));
    if !window.focused || !held || keys.just_pressed(KeyCode::Escape) {
        drag.anchor = None;
        return;
    }
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    if drag.anchor.is_none() && crate::sidebar::over_sidebar(cursor, window.width()) {
        return;
    }
    let anchor = *drag.anchor.get_or_insert(cursor);
    let (mut transform, projection) = camera.into_inner();
    if let Some(scale) = orthographic_scale(projection) {
        transform.translation += (pan_velocity(anchor, cursor)
            * PAN_SPEED
            * scale
            * options.speed_factor()
            * time.delta_secs().min(0.05))
        .extend(0.0);
    }
}

pub(crate) fn zoom_camera(
    scroll: Res<AccumulatedMouseScroll>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut projection: Single<&mut Projection, With<Camera2d>>,
) {
    if !window.focused
        || window
            .cursor_position()
            .is_none_or(|p| crate::sidebar::over_sidebar(p, window.width()))
        || scroll.delta.y == 0.0
    {
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

    #[test]
    fn held_rate_pan_continues_without_cursor_motion_or_window_edges() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<MiddleDrag>()
            .init_resource::<CameraOptions>()
            .init_resource::<Time>()
            .add_systems(Update, drag_camera);
        let mut window = Window {
            focused: true,
            ..default()
        };
        window.set_cursor_position(Some(Vec2::new(300.0, 300.0)));
        let id = app.world_mut().spawn((window, PrimaryWindow)).id();
        let camera = app
            .world_mut()
            .spawn((
                Camera2d,
                Transform::default(),
                Projection::Orthographic(OrthographicProjection::default_2d()),
            ))
            .id();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Middle);
        app.update();
        app.world_mut()
            .get_mut::<Window>(id)
            .unwrap()
            .set_cursor_position(Some(Vec2::new(360.0, 300.0)));
        for _ in 0..60 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
            app.update();
        }
        let x = app.world().get::<Transform>(camera).unwrap().translation.x;
        assert!(
            x > 700.0,
            "stationary cursor should continue scrolling: {x}"
        );
        app.world_mut().get_mut::<Window>(id).unwrap().focused = false;
        app.update();
        assert_eq!(
            app.world().get::<Transform>(camera).unwrap().translation.x,
            x
        );
        assert!(app.world().resource::<MiddleDrag>().anchor.is_none());
        assert_eq!(pan_velocity(Vec2::ZERO, Vec2::splat(2.0)), Vec2::ZERO);
        assert!(pan_velocity(Vec2::ZERO, Vec2::splat(1000.0)).length() <= 1.0001);
    }
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
        // At the very edge: full speed, the same on all four sides.
        assert_eq!(edge(0.0, 360.0), Vec2::NEG_X);
        assert_eq!(edge(1280.0, 360.0), Vec2::X);
        assert_eq!(edge(640.0, 0.0), Vec2::Y, "top of the window pans up");
        assert_eq!(edge(640.0, 720.0), Vec2::NEG_Y, "bottom pans down");
        // The band is EDGE_PAN_MARGIN pixels deep on each side; inside it the speed ramps from
        // EDGE_PAN_MIN_SPEED at its inner border up to 1, identically for every edge.
        assert_eq!(edge(EDGE_PAN_MARGIN, 360.0), Vec2::ZERO);
        assert_eq!(edge(640.0, EDGE_PAN_MARGIN), Vec2::ZERO);
        let depth = EDGE_PAN_MARGIN / 2.0;
        let left = edge(depth, 360.0);
        let right = edge(window.x - depth, 360.0);
        let top = edge(640.0, depth);
        let bottom = edge(640.0, window.y - depth);
        let speed = left.length();
        assert!(speed > EDGE_PAN_MIN_SPEED && speed < 1.0, "{speed}");
        for (v, axis) in [
            (left, Vec2::NEG_X),
            (right, Vec2::X),
            (top, Vec2::Y),
            (bottom, Vec2::NEG_Y),
        ] {
            assert!((v - axis * speed).length() < 1e-5, "{v} vs {axis}");
        }
        // A cursor a few pixels short of the top or bottom (the macOS title bar and Dock
        // trigger) already pans at nearly full speed.
        assert!(edge(640.0, 3.0).y > 0.9);
        assert!(edge(640.0, window.y - 3.0).y < -0.9);
        // Corners pan diagonally at unit speed, and near a corner the other axis joins in.
        let corner = edge(0.0, 0.0);
        assert!((corner.length() - 1.0).abs() < 1e-6);
        assert!(corner.x < 0.0 && corner.y > 0.0);
        let near_corner = edge(EDGE_CORNER_REACH - 1.0, window.y - 2.0);
        assert!(near_corner.x < 0.0 && near_corner.y < 0.0, "{near_corner}");
        assert!(near_corner.length() <= 1.0 + 1e-6);
        // Same behaviour in any window size (fullscreen, maximised, small windows).
        for size in [
            Vec2::new(800.0, 500.0),
            Vec2::new(1512.0, 945.0),
            Vec2::new(3456.0, 2234.0),
        ] {
            let at = |x: f32, y: f32| edge_direction(Vec2::new(x, y), size, EDGE_PAN_MARGIN);
            assert_eq!(at(size.x / 2.0, 1.0).x, 0.0);
            assert!(at(size.x / 2.0, 1.0).y > 0.9);
            assert!(at(size.x / 2.0, size.y - 1.0).y < -0.9);
            assert!(at(1.0, size.y / 2.0).x < -0.9);
            assert!(at(size.x - 1.0, size.y / 2.0).x > 0.9);
            assert_eq!(at(size.x / 2.0, size.y / 2.0), Vec2::ZERO);
        }
    }

    #[test]
    fn leaving_the_window_through_an_edge_keeps_panning_that_way() {
        let window = Vec2::new(1280.0, 800.0);
        // Up into the title bar, down onto the Dock, or sideways off a screen edge.
        assert_eq!(exit_direction(Vec2::new(640.0, 4.0), window), Vec2::Y);
        assert_eq!(
            exit_direction(Vec2::new(640.0, 60.0), window),
            Vec2::Y,
            "a fast flick"
        );
        assert_eq!(exit_direction(Vec2::new(640.0, 795.0), window), Vec2::NEG_Y);
        assert_eq!(exit_direction(Vec2::new(2.0, 400.0), window), Vec2::NEG_X);
        assert_eq!(exit_direction(Vec2::new(1279.0, 400.0), window), Vec2::X);
        // Through a corner: diagonal; near one edge but far along the other: that edge only.
        let corner = exit_direction(Vec2::new(3.0, 790.0), window);
        assert!(corner.x < 0.0 && corner.y < 0.0);
        assert_eq!(exit_direction(Vec2::new(150.0, 2.0), window), Vec2::Y);
        // From the middle (a jump, not an exit): no panning.
        assert_eq!(exit_direction(Vec2::new(640.0, 400.0), window), Vec2::ZERO);
    }

    #[test]
    fn panning_eases_in_and_comes_to_a_definite_stop() {
        let mut v = Vec2::ZERO;
        v = eased_velocity(v, Vec2::Y, 1.0 / 60.0);
        assert!(v.y > 0.1 && v.y < 0.6, "{v}");
        for _ in 0..30 {
            v = eased_velocity(v, Vec2::Y, 1.0 / 60.0);
        }
        assert!(v.y > 0.99);
        for _ in 0..30 {
            v = eased_velocity(v, Vec2::ZERO, 1.0 / 60.0);
        }
        assert_eq!(v, Vec2::ZERO);
    }

    /// The pan system with a window of `size` and edge scrolling on.
    fn edge_app(size: Vec2) -> (App, Entity, Entity) {
        let mut app = App::new();
        app.init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<EdgePan>()
            .insert_resource(CameraOptions {
                edge_scrolling: true,
                ..default()
            })
            .init_resource::<Time>()
            .add_systems(Update, pan_camera);
        let window = Window {
            focused: true,
            resolution: (size.x as u32, size.y as u32).into(),
            ..default()
        };
        let window = app.world_mut().spawn((window, PrimaryWindow)).id();
        let camera = app
            .world_mut()
            .spawn((
                Camera2d,
                Transform::default(),
                Projection::Orthographic(OrthographicProjection::default_2d()),
            ))
            .id();
        (app, window, camera)
    }

    fn run_frames(app: &mut App, frames: usize) {
        for _ in 0..frames {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
            app.update();
        }
    }

    #[test]
    fn edge_scrolling_is_equally_responsive_on_all_four_edges_and_diagonals() {
        let size = Vec2::new(1280.0, 800.0);
        let mut moved = Vec::new();
        for cursor in [
            Vec2::new(2.0, 400.0),
            Vec2::new(1278.0, 400.0),
            Vec2::new(640.0, 2.0),
            Vec2::new(640.0, 798.0),
            Vec2::new(2.0, 2.0),
            Vec2::new(1278.0, 798.0),
        ] {
            let (mut app, window, camera) = edge_app(size);
            app.world_mut()
                .get_mut::<Window>(window)
                .unwrap()
                .set_cursor_position(Some(cursor));
            run_frames(&mut app, 60);
            moved.push(
                app.world()
                    .get::<Transform>(camera)
                    .unwrap()
                    .translation
                    .truncate(),
            );
        }
        let distance = moved[0].length();
        assert!(distance > 700.0, "{moved:?}");
        assert!(moved[0].x < 0.0 && moved[1].x > 0.0 && moved[2].y > 0.0 && moved[3].y < 0.0);
        for m in &moved[..4] {
            assert!(
                (m.length() - distance).abs() < distance * 0.001,
                "{moved:?}"
            );
        }
        for m in &moved[4..] {
            assert!((m.length() - distance).abs() < distance * 0.06, "{moved:?}");
        }
        assert!(moved[4].x < 0.0 && moved[4].y > 0.0, "up-left diagonal");
        assert!(moved[5].x > 0.0 && moved[5].y < 0.0, "down-right diagonal");
    }

    #[test]
    fn hovering_an_interface_button_inside_an_edge_band_does_not_scroll() {
        let (mut app, window, camera) = edge_app(Vec2::new(1280.0, 800.0));
        let button = app.world_mut().spawn(Interaction::Hovered).id();
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .set_cursor_position(Some(Vec2::new(1270.0, 400.0)));
        run_frames(&mut app, 30);
        assert_eq!(
            app.world().get::<Transform>(camera).unwrap().translation.x,
            0.0
        );
        *app.world_mut().get_mut::<Interaction>(button).unwrap() = Interaction::None;
        run_frames(&mut app, 30);
        assert!(app.world().get::<Transform>(camera).unwrap().translation.x > 100.0);
    }

    #[test]
    fn the_camera_keeps_panning_while_the_cursor_is_in_the_title_bar_or_on_the_dock() {
        for (inside, sign) in [
            (Vec2::new(640.0, 10.0), 1.0),
            (Vec2::new(640.0, 790.0), -1.0),
        ] {
            let (mut app, window, camera) = edge_app(Vec2::new(1280.0, 800.0));
            app.world_mut()
                .get_mut::<Window>(window)
                .unwrap()
                .set_cursor_position(Some(inside));
            run_frames(&mut app, 2);
            // The cursor crosses the content edge: Bevy reports no cursor position.
            app.world_mut()
                .get_mut::<Window>(window)
                .unwrap()
                .set_cursor_position(None);
            let before = app.world().get::<Transform>(camera).unwrap().translation.y;
            run_frames(&mut app, 30);
            let after = app.world().get::<Transform>(camera).unwrap().translation.y;
            assert!((after - before) * sign > 400.0, "{before} -> {after}");
            // Focus loss stops it at once.
            app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
            run_frames(&mut app, 5);
            assert_eq!(
                app.world().get::<Transform>(camera).unwrap().translation.y,
                after
            );
            // Returning to the middle of the window stops it too.
            let mut w = app.world_mut().get_mut::<Window>(window).unwrap();
            w.focused = true;
            w.set_cursor_position(Some(Vec2::new(640.0, 400.0)));
            run_frames(&mut app, 30);
            let settled = app.world().get::<Transform>(camera).unwrap().translation.y;
            run_frames(&mut app, 10);
            assert_eq!(
                app.world().get::<Transform>(camera).unwrap().translation.y,
                settled
            );
        }
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
