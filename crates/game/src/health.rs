//! Health bars (PLAN.md section 7, M5; docs/DECISIONS.md D37): a bar above every damaged unit
//! and building the human player is shown, and above every selected unit.
//!
//! A bar is drawn only over a view that is shown, so the hit points of an enemy the human player
//! does not see are never drawn (as its view, D31).

use bevy::prelude::*;
use sim::{Building, Definitions, Health, Unit};

use crate::buildings::{BuildingView, WALL_PX, sprite_size};
use crate::selection::Selection;
use crate::sim_host::SimHost;
use crate::units::{FRAME_SIZE, GROUND_OFFSET_PX, UnitView};

/// Length of a unit's bar in world units; a building's bar is as wide as half its sprite.
pub const UNIT_BAR_LENGTH: f32 = 32.0;

pub struct HealthBarsPlugin;

impl Plugin for HealthBarsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            draw_health_bars
                .after(bevy::transform::TransformSystems::Propagate)
                // It reads the selection that these systems write.
                .after(crate::selection::issue_orders),
        );
    }
}

/// The filled fraction of a bar for `hit_points` of `max` (0 to 1).
pub fn health_fraction(hit_points: i32, max: i32) -> f32 {
    if max <= 0 {
        return 0.0;
    }
    (hit_points.clamp(0, max) as f32) / (max as f32)
}

/// Whether a bar is drawn: only over a shown view, for a damaged entity or a selected unit.
pub fn bar_drawn(shown: bool, hit_points: i32, max: i32, selected: bool) -> bool {
    shown && (selected || hit_points < max)
}

/// The colour of the filled part: green above half, yellow above a quarter, red below.
pub fn bar_colour(fraction: f32) -> Color {
    if fraction > 0.5 {
        Color::srgb(0.2, 0.9, 0.2)
    } else if fraction > 0.25 {
        Color::srgb(0.95, 0.85, 0.1)
    } else {
        Color::srgb(0.95, 0.2, 0.1)
    }
}

/// The two ends of the filled part of a bar of `length` centred on `centre`.
pub fn bar_fill(centre: Vec2, length: f32, fraction: f32) -> (Vec2, Vec2) {
    let left = centre - Vec2::new(length / 2.0, 0.0);
    (
        left,
        left + Vec2::new(length * fraction.clamp(0.0, 1.0), 0.0),
    )
}

fn draw_bar(gizmos: &mut Gizmos, centre: Vec2, length: f32, fraction: f32) {
    let (left, right) = bar_fill(centre, length, 1.0);
    gizmos.line_2d(left, right, Color::srgb(0.25, 0.05, 0.05));
    let (left, right) = bar_fill(centre, length, fraction);
    if right.x > left.x {
        gizmos.line_2d(left, right, bar_colour(fraction));
    }
}

/// The hit points of a unit or building type: its combat terms' maximum.
fn max_hit_points(
    definitions: &Definitions,
    unit: Option<&Unit>,
    building: Option<&Building>,
) -> Option<i32> {
    match (unit, building) {
        (Some(unit), _) => definitions.unit_combat(unit.kind),
        (None, Some(building)) => definitions.building_combat(building.kind),
        (None, None) => None,
    }
    .map(|terms| terms.hit_points)
}

fn draw_health_bars(
    host: Res<SimHost>,
    selection: Res<Selection>,
    units: Query<(&UnitView, &Transform, &Visibility)>,
    buildings: Query<(&BuildingView, &Transform, &Visibility)>,
    mut gizmos: Gizmos,
) {
    let Some(definitions) = host.definitions() else {
        return;
    };
    let world = host.world();
    let health = |entity: Entity| -> Option<(i32, i32)> {
        let hit_points = world.get::<Health>(entity)?.hit_points;
        let max = max_hit_points(
            definitions,
            world.get::<Unit>(entity),
            world.get::<Building>(entity),
        )?;
        Some((hit_points, max))
    };
    for (view, transform, visibility) in &units {
        let Some((hit_points, max)) = health(view.unit) else {
            continue;
        };
        let selected = selection.0.contains(&view.id);
        if !bar_drawn(*visibility != Visibility::Hidden, hit_points, max, selected) {
            continue;
        }
        // Above the sprite's frame; the sprite's centre lies `GROUND_OFFSET_PX` above the ground.
        let centre = transform.translation.truncate()
            + Vec2::new(0.0, FRAME_SIZE as f32 / 2.0 - GROUND_OFFSET_PX);
        draw_bar(
            &mut gizmos,
            centre,
            UNIT_BAR_LENGTH,
            health_fraction(hit_points, max),
        );
    }
    for (view, transform, visibility) in &buildings {
        let Some((hit_points, max)) = health(view.building) else {
            continue;
        };
        let Some(building) = world.get::<Building>(view.building) else {
            continue;
        };
        if !bar_drawn(*visibility != Visibility::Hidden, hit_points, max, false) {
            continue;
        }
        let (width, height) = sprite_size(building.footprint.width, building.footprint.height);
        let centre = transform.translation.truncate()
            + Vec2::new(0.0, height as f32 / 2.0 - WALL_PX as f32 / 2.0);
        draw_bar(
            &mut gizmos,
            centre,
            width as f32 / 2.0,
            health_fraction(hit_points, max),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_shows_the_fraction_of_hit_points_left() {
        assert_eq!(health_fraction(50, 100), 0.5);
        assert_eq!(health_fraction(100, 100), 1.0);
        // Below 0 (removed in the tick's cleanup) and above the maximum are clamped; a maximum of
        // 0 cannot come from the data (at least 1, D28) and draws an empty bar.
        assert_eq!(health_fraction(-5, 100), 0.0);
        assert_eq!(health_fraction(150, 100), 1.0);
        assert_eq!(health_fraction(1, 0), 0.0);
        assert_eq!(
            health_fraction(1, sim::MAX_HIT_POINTS),
            1.0 / sim::MAX_HIT_POINTS as f32
        );
        assert_eq!(
            bar_fill(Vec2::ZERO, 32.0, 0.25),
            (Vec2::new(-16.0, 0.0), Vec2::new(-8.0, 0.0))
        );
        assert_eq!(bar_fill(Vec2::ZERO, 32.0, 2.0).1, Vec2::new(16.0, 0.0));
        assert_eq!(bar_colour(1.0), bar_colour(0.51));
        assert_ne!(bar_colour(0.5), bar_colour(0.51));
        assert_ne!(bar_colour(0.25), bar_colour(0.26));
    }

    #[test]
    fn bars_are_drawn_only_over_shown_views_of_damaged_or_selected_entities() {
        // Damaged and shown: drawn; at full health only when selected.
        assert!(bar_drawn(true, 99, 100, false));
        assert!(!bar_drawn(true, 100, 100, false));
        assert!(bar_drawn(true, 100, 100, true));
        // A hidden view (an enemy the human player does not see) never has a bar.
        assert!(!bar_drawn(false, 1, 100, false));
        assert!(!bar_drawn(false, 1, 100, true));
    }
}
