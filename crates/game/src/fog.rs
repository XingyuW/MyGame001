//! Fog of war and combat on screen (PLAN.md section 7, M4; docs/DECISIONS.md D31): the map's
//! tiles are tinted by what the human player sees (shroud black, fog dark, visible cells
//! unchanged), and projectiles and instant shots are drawn. Other players' units and buildings
//! are hidden while unseen by `crate::units` and `crate::buildings`.
//!
//! As everywhere in the game, this reads simulation state and never writes it (invariant I5).

use std::collections::BTreeMap;

use bevy::prelude::*;
use bevy_ecs_tiled::prelude::{TileColor, TilePos, TiledTile};
use sim::{
    Building, CellPos, Footprint, Players, Position, SUBCELLS_PER_CELL, SimEvent, SimId, SubPos,
    Vision,
};

use crate::map::MapView;
use crate::projection::{TileSize, tile_cell, to_world};
use crate::sim_host::SimHost;
use crate::units::{GROUND_OFFSET_PX, HUMAN};

/// Frames an instant shot stays on screen.
pub const SHOT_FRAMES: u8 = 6;

pub struct FogPlugin;

impl Plugin for FogPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RecentShots>()
            .add_systems(Update, (tint_tiles, record_shots))
            .add_systems(
                PostUpdate,
                draw_combat.after(bevy::transform::TransformSystems::Propagate),
            );
    }
}

/// How a cell looks to the human player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sight {
    /// Never seen: drawn black.
    Shroud,
    /// Seen before, not now: drawn dark.
    Fog,
    /// Seen now: drawn unchanged.
    Visible,
}

/// How `cell` looks in `vision`.
pub fn sight_of(vision: &Vision, cell: CellPos) -> Sight {
    if vision.is_visible(cell) {
        Sight::Visible
    } else if vision.is_explored(cell) {
        Sight::Fog
    } else {
        Sight::Shroud
    }
}

/// The tint of a tile under `sight` (multiplied with the tile's texture).
pub fn tint(sight: Sight) -> Color {
    match sight {
        Sight::Visible => Color::WHITE,
        Sight::Fog => Color::srgb(0.4, 0.4, 0.45),
        Sight::Shroud => Color::BLACK,
    }
}

/// The human player's vision; `None` before match setup (nothing is visible then).
pub fn human_vision(players: Option<&Players>) -> Option<&Vision> {
    players?.get(HUMAN).map(|state| state.vision())
}

/// Tints every map tile by the human player's vision; a tile's colour is written only when it
/// changes, so the tilemap re-uploads only changed tiles.
fn tint_tiles(
    map: Option<Res<MapView>>,
    host: Res<SimHost>,
    mut tiles: Query<(&TilePos, &mut TileColor), With<TiledTile>>,
) {
    let Some(map) = map else {
        return;
    };
    let empty = Vision::default();
    let vision = human_vision(host.players()).unwrap_or(&empty);
    for (position, mut colour) in &mut tiles {
        let Some(cell) = tile_cell(*position, map.height) else {
            continue;
        };
        let wanted = tint(sight_of(vision, cell));
        if colour.0 != wanted {
            colour.0 = wanted;
        }
    }
}

/// Instant shots seen recently: attacker and target, and the frames left to draw them.
#[derive(Resource, Debug, Default)]
pub struct RecentShots(pub Vec<(SimId, SimId, u8)>);

/// Records the instant shots of the last ticks (a projectile weapon's shot is drawn as its
/// projectile instead).
fn record_shots(
    mut shots: ResMut<RecentShots>,
    mut events: MessageReader<SimEvent>,
    host: Res<SimHost>,
) {
    let Some(definitions) = host.definitions() else {
        return;
    };
    shots.0.retain_mut(|(.., frames)| {
        *frames = frames.saturating_sub(1);
        *frames > 0
    });
    for event in events.read() {
        if let SimEvent::WeaponFired {
            attacker,
            target,
            weapon,
        } = event
            && definitions
                .weapon(*weapon)
                .is_some_and(|def| def.projectile_speed == 0)
        {
            shots.0.push((*attacker, *target, SHOT_FRAMES));
        }
    }
}

/// The centre of a footprint in subcells.
fn footprint_centre(footprint: Footprint) -> SubPos {
    let half = SUBCELLS_PER_CELL / 2;
    SubPos::new(
        footprint
            .origin
            .x
            .saturating_mul(SUBCELLS_PER_CELL)
            .saturating_add(i32::from(footprint.width).saturating_mul(half)),
        footprint
            .origin
            .y
            .saturating_mul(SUBCELLS_PER_CELL)
            .saturating_add(i32::from(footprint.height).saturating_mul(half)),
    )
}

/// The world position of a point in subcells, raised to a unit's body.
fn body_point(position: SubPos, tile: TileSize) -> Vec2 {
    let scale = SUBCELLS_PER_CELL as f32;
    to_world(position.x as f32 / scale, position.y as f32 / scale, tile)
        + Vec2::new(0.0, GROUND_OFFSET_PX)
}

/// Where a projectile is drawn: `progress` (0 to 1) of the way from `origin` to `target`.
pub fn projectile_point(origin: SubPos, target: SubPos, progress: f32) -> SubPos {
    let progress = progress.clamp(0.0, 1.0);
    let lerp = |a: i32, b: i32| (a as f32 + (b - a) as f32 * progress).round() as i32;
    SubPos::new(lerp(origin.x, target.x), lerp(origin.y, target.y))
}

/// How an instant shot is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotView {
    /// A line from the attacker to the target.
    Line,
    /// A flash at the attacker only.
    AtAttacker,
    /// An impact at the target only.
    AtTarget,
    /// Nothing.
    Hidden,
}

/// How an instant shot is drawn, given whether the human player sees the cell of its attacker
/// and of its target: the line only when both ends are seen, else a mark at the end that is
/// seen, so that a shot never shows where an unseen unit or building stands (M4 review 1 P2-1;
/// docs/DECISIONS.md D31 item 3, D33).
pub fn shot_view(attacker_seen: bool, target_seen: bool) -> ShotView {
    match (attacker_seen, target_seen) {
        (true, true) => ShotView::Line,
        (true, false) => ShotView::AtAttacker,
        (false, true) => ShotView::AtTarget,
        (false, false) => ShotView::Hidden,
    }
}

/// Draws projectiles in flight and recent instant shots, where the human player sees them.
fn draw_combat(
    map: Option<Res<MapView>>,
    host: Res<SimHost>,
    shots: Res<RecentShots>,
    mut gizmos: Gizmos,
) {
    let (Some(map), Some(vision), Some(projectiles)) =
        (map, human_vision(host.players()), host.projectiles())
    else {
        return;
    };
    let Some(mut positions) = host
        .world()
        .try_query::<(&SimId, Option<&Position>, Option<&Building>)>()
    else {
        return;
    };
    let points: BTreeMap<SimId, SubPos> = positions
        .iter(host.world())
        .filter_map(|(id, position, building)| {
            let point = match (position, building) {
                (Some(position), _) => position.0,
                (None, Some(building)) => footprint_centre(building.footprint),
                (None, None) => return None,
            };
            Some((*id, point))
        })
        .collect();
    let seen = |point: SubPos| vision.is_visible(point.cell());
    for projectile in projectiles.iter() {
        let Some(&target) = points.get(&projectile.target) else {
            continue;
        };
        let flown = projectile.flight.saturating_sub(projectile.remaining);
        let progress = flown as f32 / projectile.flight.max(1) as f32;
        let at = projectile_point(projectile.origin, target, progress);
        if seen(at) {
            gizmos.circle_2d(body_point(at, map.tile), 3.0, Color::srgb(1.0, 0.85, 0.3));
        }
    }
    for &(attacker, target, _) in &shots.0 {
        let (Some(&from), Some(&to)) = (points.get(&attacker), points.get(&target)) else {
            continue;
        };
        let colour = Color::srgb(1.0, 1.0, 0.6);
        match shot_view(seen(from), seen(to)) {
            ShotView::Line => {
                gizmos.line_2d(body_point(from, map.tile), body_point(to, map.tile), colour);
            }
            ShotView::AtAttacker => {
                gizmos.circle_2d(body_point(from, map.tile), 4.0, colour);
            }
            ShotView::AtTarget => {
                gizmos.circle_2d(body_point(to, map.tile), 4.0, colour);
            }
            ShotView::Hidden => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_are_tinted_by_what_the_human_player_sees() {
        // Through the simulation: a unit of the human player with a sight of 5 cells.
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let data = root.join("../sim/tests/fixtures/data");
        let mut app = App::new();
        app.add_plugins(sim::SimPlugin { seed: 0 });
        let grid = sim::load_map(root.join("../sim/tests/fixtures/maps/economy_32x32.tmx"))
            .expect("map")
            .grid;
        app.world_mut().insert_resource(grid);
        app.world_mut().insert_resource(
            sim::load_match_definitions(
                data.join("units_combat.ron"),
                data.join("economy_combat.ron"),
                data.join("combat.ron"),
            )
            .expect("definitions"),
        );
        app.world_mut()
            .resource_mut::<Players>()
            .add(HUMAN, 0)
            .expect("player");
        let unit = sim::spawn_unit(app.world_mut(), HUMAN, "rifleman", CellPos::new(10, 16))
            .expect("unit");
        let vision = |app: &App| {
            app.world()
                .resource::<Players>()
                .get(HUMAN)
                .expect("player")
                .vision()
                .clone()
        };
        assert_eq!(
            sight_of(&vision(&app), CellPos::new(10, 16)),
            Sight::Shroud,
            "nothing before the first tick"
        );
        app.world_mut().run_schedule(sim::SimSchedule);
        let view = vision(&app);
        assert_eq!(sight_of(&view, CellPos::new(15, 16)), Sight::Visible);
        assert_eq!(sight_of(&view, CellPos::new(16, 16)), Sight::Shroud);
        app.world_mut()
            .resource_mut::<sim::CommandQueue>()
            .push(sim::IssuedCommand {
                player: HUMAN,
                command: sim::Command::Move {
                    units: vec![unit],
                    target: CellPos::new(2, 16),
                },
            });
        for _ in 0..100 {
            app.world_mut().run_schedule(sim::SimSchedule);
        }
        let view = vision(&app);
        assert_eq!(sight_of(&view, CellPos::new(15, 16)), Sight::Fog);
        assert_eq!(tint(Sight::Visible), Color::WHITE);
        assert_eq!(tint(Sight::Shroud), Color::BLACK);
        assert!(tint(Sight::Fog) != tint(Sight::Visible) && tint(Sight::Fog) != Color::BLACK);
    }

    #[test]
    fn enemies_are_shown_exactly_while_the_human_player_sees_them() {
        use crate::buildings::building_shown;
        use crate::units::{OPPONENT, unit_shown};
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let data = root.join("../sim/tests/fixtures/data");
        let mut app = App::new();
        app.add_plugins(sim::SimPlugin { seed: 0 });
        let grid = sim::load_map(root.join("../sim/tests/fixtures/maps/economy_32x32.tmx"))
            .expect("map")
            .grid;
        app.world_mut().insert_resource(grid);
        app.world_mut().insert_resource(
            sim::load_match_definitions(
                data.join("units_combat.ron"),
                data.join("economy_combat.ron"),
                data.join("combat.ron"),
            )
            .expect("definitions"),
        );
        for player in [HUMAN, OPPONENT] {
            app.world_mut()
                .resource_mut::<Players>()
                .add(player, 0)
                .expect("player");
        }
        let at = |x, y| CellPos::new(x, y);
        // The crawler stands at the edge of the rifleman's sight (5 cells); the far one beyond.
        sim::spawn_unit(app.world_mut(), HUMAN, "rifleman", at(10, 16)).expect("own");
        sim::spawn_unit(app.world_mut(), OPPONENT, "crawler", at(15, 16)).expect("near");
        sim::spawn_unit(app.world_mut(), OPPONENT, "crawler", at(16, 16)).expect("far");
        sim::spawn_building(app.world_mut(), OPPONENT, "power", at(14, 20)).expect("seen");
        sim::spawn_building(app.world_mut(), OPPONENT, "power", at(26, 20)).expect("unseen");
        let shown = |app: &mut App| {
            let world = app.world_mut();
            let players = world.resource::<Players>().clone();
            let mut units = world.query::<(&sim::Owner, &sim::Mobile)>();
            let units: Vec<(CellPos, bool)> = units
                .iter(world)
                .map(|(owner, mobile)| (mobile.cell, unit_shown(owner.0, mobile, &players)))
                .collect();
            let mut buildings = world.query::<(&sim::Owner, &Building)>();
            let buildings: Vec<(CellPos, bool)> = buildings
                .iter(world)
                .map(|(owner, building)| {
                    (
                        building.footprint.origin,
                        building_shown(owner.0, building.footprint, &players),
                    )
                })
                .collect();
            let mut all: Vec<(CellPos, bool)> = units.into_iter().chain(buildings).collect();
            all.sort();
            all
        };
        // Before the first tick only the human player's own unit is shown.
        assert_eq!(
            shown(&mut app),
            [
                (at(10, 16), true),
                (at(14, 20), false),
                (at(15, 16), false),
                (at(16, 16), false),
                (at(26, 20), false)
            ]
        );
        app.world_mut().run_schedule(sim::SimSchedule);
        // The near crawler is shown, the far one is not, and the plant whose nearest cell
        // (14, 20) is 4 cells away along both axes (32 > 25) stays hidden.
        assert_eq!(
            shown(&mut app),
            [
                (at(10, 16), true),
                (at(14, 20), false),
                (at(15, 16), true),
                (at(16, 16), false),
                (at(26, 20), false)
            ]
        );
    }

    #[test]
    fn the_fog_systems_share_no_state_with_other_game_systems_without_an_order() {
        // As `selection::tests::no_two_game_systems_share_state_in_post_update_without_an_order`,
        // with the fog plugin, for both schedules it adds systems to.
        let mut app = App::new();
        app.add_plugins((
            crate::map::MapPlugin,
            crate::units::UnitsPlugin,
            crate::buildings::BuildingsPlugin,
            crate::selection::SelectionPlugin,
            crate::construction::ConstructionPlugin,
            FogPlugin,
        ));
        let check = |world: &mut World, schedule: &mut Schedule| {
            schedule.initialize(world).expect("schedule builds");
            assert!(schedule.graph().conflicting_systems().is_empty());
        };
        app.world_mut().schedule_scope(PostUpdate, check);
        app.world_mut().schedule_scope(Update, check);
    }

    #[test]
    fn an_instant_shot_shows_only_the_ends_the_human_player_sees() {
        // M4 review 1 P2-1: a line from an unseen attacker would show where it stands.
        assert_eq!(shot_view(true, true), ShotView::Line);
        assert_eq!(shot_view(false, true), ShotView::AtTarget);
        assert_eq!(shot_view(true, false), ShotView::AtAttacker);
        assert_eq!(shot_view(false, false), ShotView::Hidden);
    }

    #[test]
    fn a_projectile_is_drawn_along_its_way() {
        let origin = SubPos::new(0, 0);
        let target = SubPos::new(256, -512);
        assert_eq!(projectile_point(origin, target, 0.0), origin);
        assert_eq!(projectile_point(origin, target, 1.0), target);
        assert_eq!(
            projectile_point(origin, target, 0.5),
            SubPos::new(128, -256)
        );
        assert_eq!(projectile_point(origin, target, 9.0), target);
        assert_eq!(
            footprint_centre(Footprint {
                origin: CellPos::new(2, 3),
                width: 3,
                height: 2,
            }),
            SubPos::new(2 * 256 + 384, 3 * 256 + 256)
        );
    }
}
