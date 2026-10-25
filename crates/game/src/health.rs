//! Health bars (PLAN.md section 7, M5; docs/DECISIONS.md D37): a bar above every damaged unit
//! and building the human player is shown, and above every selected unit.
//!
//! A building being repaired (`sim::Command::RepairBuilding`) also shows a pulsing wrench beside
//! its bar, whichever player repairs it.
//!
//! A bar is drawn only over a view that is shown, so the hit points of an enemy the human player
//! does not see are never drawn (as its view, D31).
//!
//! Since RA4 (docs/DECISIONS.md R52) a veteran shows one chevron and an elite unit three under
//! the right end of where its bar is, whether it is damaged or not, and a building with
//! aircraft docks shows a small diamond on each of them, brighter where an aircraft holds it.
//!
//! Infantry on foot stand three to a cell, each at a spot of its own (RA3, docs/DECISIONS.md
//! R6), so a soldier's bar is half as long as a vehicle's: the bars of the soldiers of a cell
//! do not overlap.

use bevy::prelude::*;
use sim::{Building, Definitions, Health, Unit};

use crate::buildings::BuildingView;
use crate::selection::Selection;
use crate::sim_host::SimHost;
use crate::units::{FRAME_SIZE, GROUND_OFFSET_PX, UnitView};

/// Length of a unit's bar in world units; a building's bar is as wide as half its sprite.
pub const UNIT_BAR_LENGTH: f32 = 32.0;

/// Length of the bar of a unit on foot, which shares its cell with two others.
pub const INFANTRY_BAR_LENGTH: f32 = 16.0;

/// The length of a unit's bar: shorter for a unit on foot (`sim::Definitions::on_foot`).
pub fn unit_bar_length(on_foot: bool) -> f32 {
    if on_foot {
        INFANTRY_BAR_LENGTH
    } else {
        UNIT_BAR_LENGTH
    }
}

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

/// How many chevrons a unit of `rank` shows (RA4): one for a veteran, three for an elite unit.
pub fn chevron_count(rank: u8) -> usize {
    match rank {
        0 => 0,
        1 => 1,
        _ => 3,
    }
}

/// The chevrons of a unit of `rank` whose bar is centred on `centre` and `length` long: each
/// three points (left end, tip, right end), stacked upwards under the right end of the bar.
pub fn rank_chevrons(rank: u8, centre: Vec2, length: f32) -> Vec<[Vec2; 3]> {
    let base = centre + Vec2::new(length / 2.0 - 4.0, -11.0);
    (0..chevron_count(rank))
        .map(|n| {
            let tip = base + Vec2::new(0.0, 3.0 * n as f32);
            [tip + Vec2::new(-3.5, -2.5), tip, tip + Vec2::new(3.5, -2.5)]
        })
        .collect()
}

/// The colour of the chevrons of `rank`: gold for a veteran, pale gold for an elite unit.
pub fn chevron_colour(rank: u8) -> Color {
    if rank >= sim::ELITE {
        Color::srgb(1.0, 0.95, 0.62)
    } else {
        Color::srgb(1.0, 0.8, 0.22)
    }
}

/// The outline of the mark of a dock whose cell centre is at the ground point `at` (RA4): a
/// closed diamond, as flat as a cell.
pub fn dock_mark(at: Vec2) -> [Vec2; 5] {
    let (w, h) = (9.0, 4.5);
    [
        at + Vec2::new(-w, 0.0),
        at + Vec2::new(0.0, h),
        at + Vec2::new(w, 0.0),
        at + Vec2::new(0.0, -h),
        at + Vec2::new(-w, 0.0),
    ]
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

/// Draws a small wrench centred on `centre`, its colour pulsing with `phase` (seconds).
fn draw_wrench(gizmos: &mut Gizmos, centre: Vec2, phase: f32) {
    let glow = 0.75 + 0.25 * (phase * std::f32::consts::TAU * 1.5).sin();
    let colour = Color::srgb(0.35 * glow, 0.95 * glow, 0.45 * glow);
    let dark = Color::srgb(0.02, 0.08, 0.03);
    let axis = Vec2::new(1.0, -1.0).normalize();
    let head = centre - axis * 4.5;
    let tail = centre + axis * 6.0;
    // Dark outline first so the wrench reads over snow and concrete alike.
    for offset in [Vec2::new(1.0, 0.0), Vec2::new(0.0, -1.0)] {
        gizmos.line_2d(head + offset, tail + offset, dark);
        gizmos.circle_2d(head + offset, 3.5, dark);
    }
    gizmos.line_2d(head, tail, colour);
    gizmos.line_2d(
        head + Vec2::new(0.5, 0.5),
        tail + Vec2::new(0.5, 0.5),
        colour,
    );
    gizmos.circle_2d(head, 3.5, colour);
    // The jaw's opening.
    let jaw = head - axis * 3.5;
    gizmos.line_2d(jaw + Vec2::new(-1.5, -1.5), jaw + Vec2::new(1.5, 1.5), dark);
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
    time: Res<Time<Real>>,
    map: Option<Res<crate::map::MapView>>,
    selection: Res<Selection>,
    units: Query<(&UnitView, &Transform, &Visibility)>,
    buildings: Query<(&BuildingView, &Transform, &Visibility)>,
    mut gizmos: Gizmos,
) {
    let Some(definitions) = host.definitions() else {
        return;
    };
    let world = host.world();
    let repairing: std::collections::BTreeSet<sim::SimId> = host
        .players()
        .map(|players| {
            players
                .iter()
                .flat_map(|(_, state)| state.repairing().iter().copied())
                .collect()
        })
        .unwrap_or_default();
    let phase = time.elapsed_secs();
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
        let shown = *visibility != Visibility::Hidden;
        // Above the sprite's frame; the sprite's centre lies `GROUND_OFFSET_PX` above the ground.
        let centre = transform.translation.truncate()
            + Vec2::new(0.0, FRAME_SIZE as f32 / 2.0 - GROUND_OFFSET_PX);
        let on_foot = world
            .get::<Unit>(view.unit)
            .is_some_and(|unit| definitions.on_foot(unit.kind));
        let length = unit_bar_length(on_foot);
        // RA4: the insignia of a veteran or elite unit, over every such unit that is shown.
        let rank = world
            .get::<sim::Experience>(view.unit)
            .map_or(0, |experience| experience.rank);
        if shown && rank > 0 {
            let dark = Color::srgb(0.1, 0.07, 0.0);
            for [left, tip, right] in rank_chevrons(rank, centre, length) {
                for (a, b) in [(left, tip), (tip, right)] {
                    gizmos.line_2d(a - Vec2::Y, b - Vec2::Y, dark);
                    gizmos.line_2d(a, b, chevron_colour(rank));
                }
            }
        }
        let selected = selection.0.contains(&view.id);
        if !bar_drawn(shown, hit_points, max, selected) {
            continue;
        }
        draw_bar(
            &mut gizmos,
            centre,
            length,
            health_fraction(hit_points, max),
        );
    }
    // RA4: the docks of the buildings that hold aircraft, each above its cell.
    let taken: std::collections::BTreeSet<(sim::SimId, u8)> = world
        .get_resource::<sim::SpecialState>()
        .map(|state| state.docks.values().copied().collect())
        .unwrap_or_default();
    for (view, transform, visibility) in &buildings {
        let Some((hit_points, max)) = health(view.building) else {
            continue;
        };
        let Some(building) = world.get::<Building>(view.building) else {
            continue;
        };
        let docks = definitions
            .metadata(sim::ProductionItem::Building(building.kind))
            .map_or(0, |entry| entry.docks);
        if *visibility != Visibility::Hidden
            && docks > 0
            && let Some(map) = map.as_deref()
        {
            for dock in 0..docks.min(u32::from(u8::MAX)) as u8 {
                let cell = sim::dock_cell(building.footprint, dock);
                let at = crate::units::ground_point(
                    Vec2::new(cell.x as f32 + 0.5, cell.y as f32 + 0.5),
                    map.tile,
                );
                let colour = if taken.contains(&(view.id, dock)) {
                    Color::srgba(1.0, 0.95, 0.7, 0.95)
                } else {
                    Color::srgba(0.95, 0.8, 0.25, 0.6)
                };
                let outline = dock_mark(at);
                for pair in outline.windows(2) {
                    gizmos.line_2d(pair[0], pair[1], colour);
                }
            }
        }
        if !bar_drawn(*visibility != Visibility::Hidden, hit_points, max, false) {
            continue;
        }
        let size =
            crate::hd_art::building_size(building.footprint.width, building.footprint.height);
        let (width, height) = (size.x, size.y);
        let centre = transform.translation.truncate() + Vec2::new(0.0, height / 2.0 - 16.0);
        draw_bar(
            &mut gizmos,
            centre,
            width / 2.0,
            health_fraction(hit_points, max),
        );
        if repairing.contains(&view.id) {
            draw_wrench(
                &mut gizmos,
                centre + Vec2::new(width / 4.0 + 10.0, 0.0),
                phase,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RA4: a veteran shows one chevron and an elite unit three, under the right end of its
    /// bar and within the bar's width, also for a soldier's short bar; a dock is marked by a
    /// closed diamond around its ground point.
    #[test]
    fn the_insignia_fit_under_the_bar_and_a_dock_is_a_closed_diamond() {
        assert_eq!(chevron_count(0), 0);
        assert_eq!(chevron_count(sim::VETERAN), 1);
        assert_eq!(chevron_count(sim::ELITE), 3);
        assert_ne!(chevron_colour(sim::VETERAN), chevron_colour(sim::ELITE));
        let centre = Vec2::new(100.0, 50.0);
        for length in [UNIT_BAR_LENGTH, INFANTRY_BAR_LENGTH] {
            assert!(rank_chevrons(0, centre, length).is_empty());
            for rank in [sim::VETERAN, sim::ELITE] {
                let chevrons = rank_chevrons(rank, centre, length);
                assert_eq!(chevrons.len(), chevron_count(rank));
                for [left, tip, right] in &chevrons {
                    // A chevron points up, is symmetric, and lies below the bar inside its span.
                    assert!(tip.y > left.y && left.y == right.y);
                    assert_eq!(tip.x - left.x, right.x - tip.x);
                    assert!(
                        left.x >= centre.x - length / 2.0 && right.x <= centre.x + length / 2.0
                    );
                    assert!(tip.y < centre.y - 2.0);
                }
                // Stacked: no two at one height.
                for pair in chevrons.windows(2) {
                    assert!(pair[1][1].y - pair[0][1].y >= 3.0);
                }
            }
        }
        let at = Vec2::new(10.0, 20.0);
        let mark = dock_mark(at);
        assert_eq!(mark[0], mark[4], "closed");
        let (sum, count) = mark[..4]
            .iter()
            .fold((Vec2::ZERO, 0.0), |(sum, count), point| {
                (sum + *point, count + 1.0)
            });
        assert_eq!(sum / count, at, "around the ground point");
        // As flat as a cell is drawn: twice as wide as high.
        assert_eq!(mark[2].x - mark[0].x, 2.0 * (mark[1].y - mark[3].y));
    }

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

    /// RA3: three soldiers stand in one cell, each at its spot; their bars, each drawn the same
    /// distance above its soldier, do not overlap, and a click or the cursor finds the soldier
    /// it is on (`crate::selection::unit_at`, which the hover label uses too).
    #[test]
    fn the_bars_of_three_soldiers_in_a_cell_do_not_overlap_and_each_can_be_picked() {
        use crate::projection::{TileSize, to_world};
        let tile = TileSize::new(sim::TILE_WIDTH_PX, sim::TILE_HEIGHT_PX);
        let cell = sim::CellPos::new(7, 9);
        let scale = sim::SUBCELLS_PER_CELL as f32;
        let ground: Vec<Vec2> = (0..sim::SPOTS_PER_CELL as u8)
            .map(|spot| {
                let at = sim::SubPos::spot_of(cell, spot);
                to_world(at.x as f32 / scale, at.y as f32 / scale, tile)
            })
            .collect();
        assert_eq!(unit_bar_length(true), INFANTRY_BAR_LENGTH);
        assert_eq!(unit_bar_length(false), UNIT_BAR_LENGTH);
        let bar = |at: Vec2| bar_fill(at, unit_bar_length(true), 1.0);
        for (index, &a) in ground.iter().enumerate() {
            for &b in &ground[index + 1..] {
                // Bars are horizontal lines: two overlap if they are at one height (within the
                // two pixels a line may cover) and their spans meet.
                let ((a0, a1), (b0, b1)) = (bar(a), bar(b));
                let level = (a.y - b.y).abs() < 2.0;
                let apart = a1.x + 2.0 <= b0.x || b1.x + 2.0 <= a0.x;
                assert!(!level || apart, "bars at {a} and {b} overlap");
                // The soldiers themselves are far enough apart to tell them from each other.
                assert!(a.distance(b) >= 12.0, "{a} and {b}");
            }
        }
        // A vehicle's bar would not fit: the two soldiers that stand side by side are closer
        // together than it is long.
        assert!((ground[1].x - ground[2].x).abs() < UNIT_BAR_LENGTH);
        // On screen the first stands above the centre and the other two below it, left and
        // right of it.
        let centre = crate::projection::cell_centre(cell, tile);
        assert!(ground[0].y > centre.y && (ground[0].x - centre.x).abs() < 0.01);
        assert!(ground[1].x > centre.x && ground[2].x < centre.x);
        assert!((ground[1].y - ground[2].y).abs() < 0.01 && ground[1].y < centre.y);
        // Picking: the cursor on the body of each soldier picks that soldier.
        let units: Vec<(sim::SimId, Vec2)> = ground
            .iter()
            .enumerate()
            .map(|(index, &at)| (sim::SimId(index as u32 + 1), at))
            .collect();
        for &(id, at) in &units {
            let body = at + Vec2::new(0.0, GROUND_OFFSET_PX);
            assert_eq!(crate::selection::unit_at(body, &units), Some(id));
            // A little off its centre, on the side away from the others, too.
            let away = (at - centre).normalize_or_zero() * 4.0;
            assert_eq!(crate::selection::unit_at(body + away, &units), Some(id));
        }
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
