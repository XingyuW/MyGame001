//! The name of what lies under the cursor: hovering a unit or building the human player is
//! shown (their own or an enemy's) shows a small label above the cursor with its name, owner and
//! health, so similar-looking miniatures can always be told apart.
//!
//! Like the health bars (`crate::health`), only shown views are named, so the label reveals
//! nothing that fog of war hides.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use sim::{Building, Definitions, Health, Owner, PlayerId, ProductionItem, SimId, Unit};

use crate::buildings::building_shown;
use crate::construction::InterfaceMode;
use crate::map::HoveredCell;
use crate::selection::unit_at;
use crate::sidebar::over_sidebar;
use crate::sim_host::SimHost;
use crate::units::{GROUND_OFFSET_PX, HUMAN, UnitView};

pub struct HoverInfoPlugin;

impl Plugin for HoverInfoPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            spawn_hover_label.after(crate::skirmish::load_catalogue),
        )
        .add_systems(
            PostUpdate,
            show_hover_label
                .after(bevy::transform::TransformSystems::Propagate)
                .after(bevy::camera::CameraUpdateSystems)
                .after(crate::map::update_hovered_cell)
                // They write other labels' text and nodes, which Bevy cannot tell apart from
                // this label's.
                .after(crate::selection::draw_selection)
                .after(crate::construction::update_placement_ghost),
        );
    }
}

/// The label naming what is under the cursor.
#[derive(Component, Debug)]
pub struct HoverLabel;

/// The display name of a unit or building type: its arsenal label (e.g. "Grizzly Battle Tank"),
/// or else its id in words with capitals ("heavy_tank" becomes "Heavy Tank").
pub fn display_name(definitions: &Definitions, item: ProductionItem) -> String {
    if let Some(entry) = definitions.metadata(item) {
        return entry.label.clone();
    }
    let id = match item {
        ProductionItem::Building(kind) => definitions.building(kind).map(|def| def.id.as_str()),
        ProductionItem::Unit(kind) => definitions.unit(kind).map(|def| def.id.as_str()),
    }
    .unwrap_or("?");
    id.split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The label's text: the name, then whose it is and its health in percent.
pub fn label_text(name: &str, owner: PlayerId, health: Option<(i32, i32)>) -> String {
    let side = if owner == HUMAN { "Yours" } else { "Enemy" };
    match health {
        Some((hit_points, max)) if max > 0 => {
            let percent = (i64::from(hit_points.clamp(0, max)) * 100 / i64::from(max)).max(1);
            format!("{name}\n{side}  {percent}%")
        }
        _ => format!("{name}\n{side}"),
    }
}

/// The label of scenery (`sim::Props`): its name, that it can be destroyed, and its health.
pub fn scenery_text(kind: sim::PropKind, hit_points: i32) -> String {
    let max = kind.hit_points().max(1);
    let percent = (i64::from(hit_points.clamp(0, max)) * 100 / i64::from(max)).max(1);
    format!("{}\nDestructible  {percent}%", kind.name())
}

fn spawn_hover_label(mut commands: Commands) {
    commands.spawn((
        HoverLabel,
        Text::new(""),
        TextFont::from_font_size(12.0),
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            display: Display::None,
            padding: UiRect::axes(Val::Px(5.0), Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.04, 0.05, 0.06, 0.82)),
        GlobalZIndex(6),
        bevy::ui::FocusPolicy::Pass,
        Pickable::IGNORE,
    ));
}

/// What the cursor rests on: a shown unit (its body), else a shown building (its cell).
fn hovered_entity(
    host: &SimHost,
    point: Option<Vec2>,
    cell: Option<sim::CellPos>,
    views: &Query<(&UnitView, &Transform, &Visibility)>,
) -> Option<(Entity, ProductionItem, PlayerId)> {
    let world = host.world();
    let shown: Vec<(SimId, Vec2)> = views
        .iter()
        .filter(|(_, _, visibility)| **visibility != Visibility::Hidden)
        .map(|(view, transform, _)| {
            (
                view.id,
                transform.translation.truncate() - Vec2::new(0.0, GROUND_OFFSET_PX),
            )
        })
        .collect();
    if let Some(id) = point.and_then(|point| unit_at(point, &shown))
        && let Some((view, ..)) = views.iter().find(|(view, ..)| view.id == id)
        && let Some(unit) = world.get::<Unit>(view.unit)
    {
        return Some((view.unit, ProductionItem::Unit(unit.kind), view.owner));
    }
    let id = host.grid()?.building(cell?)?;
    let players = host.players()?;
    let mut buildings = world.try_query::<(Entity, &SimId, &Owner, &Building)>()?;
    buildings
        .iter(world)
        .find(|(_, other, ..)| **other == id)
        .filter(|(_, _, owner, building)| building_shown(owner.0, building.footprint, players))
        .map(|(entity, _, owner, building)| {
            (entity, ProductionItem::Building(building.kind), owner.0)
        })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn show_hover_label(
    host: Res<SimHost>,
    hovered: Res<HoveredCell>,
    mode: Res<InterfaceMode>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform)>,
    views: Query<(&UnitView, &Transform, &Visibility)>,
    mut label: Single<(&mut Text, &mut Node, &mut TextColor), With<HoverLabel>>,
) {
    let (camera, camera_transform) = *camera;
    let cursor = window
        .cursor_position()
        .filter(|&cursor| window.focused && !over_sidebar(cursor, window.width()));
    let found = cursor
        .filter(|_| host.running() && !matches!(*mode, InterfaceMode::Place(_)))
        .and_then(|cursor| {
            let definitions = host.definitions()?;
            let point = camera.viewport_to_world_2d(camera_transform, cursor).ok();
            let (entity, item, owner) = hovered_entity(&host, point, hovered.0, &views)?;
            let world = host.world();
            let max = match item {
                ProductionItem::Unit(kind) => definitions.unit_combat(kind),
                ProductionItem::Building(kind) => definitions.building_combat(kind),
            }
            .map(|terms| terms.hit_points);
            let health = world
                .get::<Health>(entity)
                .map(|health| health.hit_points)
                .zip(max);
            Some((
                label_text(&display_name(definitions, item), owner, health),
                if owner == HUMAN {
                    Color::srgb(0.72, 0.9, 1.0)
                } else {
                    Color::srgb(1.0, 0.72, 0.66)
                },
                cursor,
            ))
        })
        .or_else(|| {
            // Scenery the player has explored: it belongs to nobody and can be destroyed.
            let cursor = cursor.filter(|_| host.running() && *mode == InterfaceMode::Normal)?;
            let cell = hovered.0?;
            let explored = host.players()?.get(HUMAN)?.vision().is_explored(cell);
            let prop = host.props()?.at(cell).filter(|_| explored)?;
            Some((
                scenery_text(prop.kind, prop.hit_points),
                Color::srgb(0.86, 0.84, 0.7),
                cursor,
            ))
        });
    let (text, node, colour) = &mut *label;
    match found {
        Some((value, tint, cursor)) => {
            if text.0 != value {
                text.0 = value;
            }
            colour.0 = tint;
            node.display = Display::Flex;
            // Above and to the right of the cursor; the order label sits below it.
            node.left = Val::Px(cursor.x + 16.0);
            node.top = Val::Px(cursor.y - 40.0);
        }
        None => {
            if node.display != Display::None {
                node.display = Display::None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenery_labels_name_the_kind_and_health() {
        assert_eq!(
            scenery_text(sim::PropKind::Trees, 60),
            "Trees\nDestructible  100%"
        );
        assert_eq!(
            scenery_text(sim::PropKind::Tower, 250),
            "Tower\nDestructible  25%"
        );
    }

    #[test]
    fn labels_name_the_type_owner_and_health() {
        assert_eq!(
            label_text("Grizzly Battle Tank", HUMAN, Some((150, 300))),
            "Grizzly Battle Tank\nYours  50%"
        );
        assert_eq!(
            label_text("Rhino Heavy Tank", PlayerId(1), Some((1, 400))),
            "Rhino Heavy Tank\nEnemy  1%",
            "a living unit never reads 0%"
        );
        assert_eq!(label_text("Ore", PlayerId(2), None), "Ore\nEnemy");
    }

    #[test]
    fn every_shipped_type_has_a_readable_name() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let (loaded, _) = crate::units::load_match_data(&root).expect("match data");
        let definitions = &loaded.definitions;
        let mut names = std::collections::BTreeSet::new();
        let units =
            (0..definitions.units().len()).map(|i| ProductionItem::Unit(sim::UnitKind(i as u16)));
        let buildings = (0..definitions.buildings().len())
            .map(|i| ProductionItem::Building(sim::BuildingKind(i as u16)));
        for item in units.chain(buildings) {
            let name = display_name(definitions, item);
            assert!(!name.is_empty() && !name.contains('_'), "{name}");
            assert!(
                name.chars().next().is_some_and(char::is_uppercase),
                "{name}"
            );
            names.insert(name);
        }
        assert!(names.len() > 60, "{} names", names.len());
    }
}
