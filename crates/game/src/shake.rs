//! Screen shake: the camera jolts when something the human player sees blows up.
//!
//! Destructions are reported here by `crate::battlefield` only after its visibility check, so a
//! shake never tells the player about anything hidden. Each one adds "trauma" weighted by how
//! close it is to the middle of the view (nothing beyond [`REACH`] half-views away); the camera
//! is displaced by trauma² × [`MAX_OFFSET_PX`] (scaled by the zoom) and trauma decays at
//! [`DECAY_PER_SEC`]. Trauma ages in game time, so a paused match stands still.
//!
//! The offset is added in `PostUpdate`, just before transforms propagate, and removed again in
//! `First`, so every input, selection and camera system in `Update` sees the camera exactly where
//! the player put it. Presentation only: nothing here touches the simulation (invariant I5).

use bevy::prelude::*;

/// Largest displacement at full trauma, in screen pixels.
pub const MAX_OFFSET_PX: f32 = 9.0;
/// Trauma lost per game second.
pub const DECAY_PER_SEC: f32 = 2.2;
/// Trauma of a destroyed building in the middle of the view.
pub const BUILDING_TRAUMA: f32 = 0.5;
/// Trauma of a destroyed unit in the middle of the view.
pub const UNIT_TRAUMA: f32 = 0.16;
/// Beyond this many half-views from the centre a destruction does not shake the view.
pub const REACH: f32 = 1.4;

pub struct ShakePlugin;

impl Plugin for ShakePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScreenShake>()
            .add_systems(First, remove_shake)
            .add_systems(
                PostUpdate,
                apply_shake.before(bevy::transform::TransformSystems::Propagate),
            );
    }
}

/// Pending jolts and the current trauma.
#[derive(Resource, Debug, Default)]
pub struct ScreenShake {
    trauma: f32,
    pending: Vec<(Vec2, f32)>,
    applied: Vec2,
    phase: f32,
}

impl ScreenShake {
    /// Reports a visible explosion of `strength` (0..1) at world point `at`.
    pub fn jolt(&mut self, at: Vec2, strength: f32) {
        self.pending.push((at, strength));
    }
}

/// How much of a jolt at `offset` from the view centre reaches the camera, given half the view's
/// world size: 1 inside the view, falling to 0 at [`REACH`] half-views.
pub fn falloff(offset: Vec2, half_view: Vec2) -> f32 {
    let half = half_view.max(Vec2::splat(1.0));
    let d = (offset / half).abs().max_element();
    if d <= 1.0 {
        1.0
    } else {
        ((REACH - d) / (REACH - 1.0)).clamp(0.0, 1.0)
    }
}

/// Adds the jolts to the trauma, then lets it decay over `dt` game seconds.
pub fn next_trauma(trauma: f32, jolts: impl IntoIterator<Item = f32>, dt: f32) -> f32 {
    let added: f32 = jolts.into_iter().sum();
    ((trauma + added).min(1.0) - DECAY_PER_SEC * dt).max(0.0)
}

fn remove_shake(
    mut shake: ResMut<ScreenShake>,
    camera: Option<Single<&mut Transform, With<Camera2d>>>,
) {
    if let Some(mut transform) = camera {
        transform.translation -= shake.applied.extend(0.0);
    }
    shake.applied = Vec2::ZERO;
}

#[allow(clippy::type_complexity)]
fn apply_shake(
    mut shake: ResMut<ScreenShake>,
    clock: Res<crate::game_speed::GameClock>,
    camera: Option<Single<(&mut Transform, &Projection, &Camera), With<Camera2d>>>,
) {
    let Some(camera) = camera else {
        shake.pending.clear();
        return;
    };
    let (mut transform, projection, camera) = camera.into_inner();
    let scale = match projection {
        Projection::Orthographic(ortho) => ortho.scale,
        _ => 1.0,
    };
    let viewport = camera
        .logical_viewport_size()
        .unwrap_or(Vec2::new(1280.0, 720.0));
    let half_view = viewport * scale / 2.0;
    let centre = transform.translation.truncate();
    let pending = std::mem::take(&mut shake.pending);
    let jolts = pending
        .iter()
        .map(|(at, strength)| strength * falloff(*at - centre, half_view));
    let dt = clock.delta();
    shake.trauma = next_trauma(shake.trauma, jolts, dt);
    shake.phase += dt;
    if shake.trauma <= 0.0 {
        return;
    }
    let amount = shake.trauma * shake.trauma * MAX_OFFSET_PX * scale;
    let t = shake.phase * 38.0;
    let offset = Vec2::new(
        (t * 1.0).sin() * 0.6 + (t * 2.3 + 1.7).sin() * 0.4,
        (t * 1.3 + 0.5).sin() * 0.6 + (t * 2.9 + 3.1).sin() * 0.4,
    ) * amount;
    transform.translation += offset.extend(0.0);
    shake.applied = offset;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jolts_in_view_count_fully_and_far_ones_not_at_all() {
        let half = Vec2::new(640.0, 360.0);
        assert_eq!(falloff(Vec2::ZERO, half), 1.0);
        assert_eq!(falloff(Vec2::new(600.0, -300.0), half), 1.0);
        assert_eq!(falloff(Vec2::new(640.0 * REACH + 1.0, 0.0), half), 0.0);
        let mid = falloff(Vec2::new(640.0 * 1.2, 0.0), half);
        assert!(mid > 0.0 && mid < 1.0);
    }

    #[test]
    fn trauma_is_capped_and_decays_to_rest() {
        let t = next_trauma(0.0, [0.8, 0.8], 0.0);
        assert_eq!(t, 1.0);
        let mut t = t;
        for _ in 0..120 {
            t = next_trauma(t, [], 1.0 / 60.0);
        }
        assert_eq!(t, 0.0);
        // Paused (no game time passes): it holds.
        assert_eq!(next_trauma(0.4, [], 0.0), 0.4);
    }

    #[test]
    fn the_offset_is_removed_before_update_and_restored_after() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::transform::TransformPlugin)
            .init_resource::<crate::game_speed::GameClock>()
            .add_plugins(ShakePlugin);
        let camera = app
            .world_mut()
            .spawn((
                Camera2d,
                Projection::Orthographic(OrthographicProjection::default_2d()),
                Transform::from_xyz(100.0, 50.0, 0.0),
            ))
            .id();
        app.world_mut().resource_mut::<ScreenShake>().trauma = 1.0;
        app.world_mut().resource_mut::<ScreenShake>().phase = 0.3;
        app.update();
        let shaken = app.world().get::<Transform>(camera).unwrap().translation;
        assert_ne!(shaken.truncate(), Vec2::new(100.0, 50.0));
        // Next frame's `First` puts it back before anything else reads it.
        app.world_mut().run_schedule(First);
        let rest = app.world().get::<Transform>(camera).unwrap().translation;
        assert_eq!(rest.truncate(), Vec2::new(100.0, 50.0));
    }
}
