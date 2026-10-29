//! The name of what lies under the cursor: hovering a unit or building the human player is
//! shown (their own or an enemy's) shows a small label above the cursor with its name, owner and
//! health, so similar-looking miniatures can always be told apart. Another player is named by
//! its team colour and the country it plays, "Gold Player (Iraq)" (RA1).
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

/// An id in words with capitals: `heavy_tank` becomes `Heavy Tank`, `allied` becomes `Allied`.
pub fn title(id: &str) -> String {
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
    title(id)
}

/// The label's text: the name, then whose it is and its health in percent. The human player's
/// own are `Yours`; another player's are named by `whose`, the player's name with the country
/// it plays (`Gold Player (Iraq)`, `crate::skirmish::player_name`), or `Enemy` without a name.
pub fn label_text(
    name: &str,
    owner: PlayerId,
    whose: Option<&str>,
    health: Option<(i32, i32)>,
) -> String {
    let side = match whose {
        _ if owner == sim::NEUTRAL => "Neutral",
        _ if owner == HUMAN => "Yours",
        Some(whose) => whose,
        None => "Enemy",
    };
    match health {
        Some((hit_points, max)) if max > 0 => {
            let percent = (i64::from(hit_points.clamp(0, max)) * 100 / i64::from(max)).max(1);
            format!("{name}\n{side}  {percent}%")
        }
        _ => format!("{name}\n{side}"),
    }
}

/// The line of a label that names the armour type of `item` (RA3), e.g. `Heavy armour`; the
/// type `none` reads `No armour`. `None` without combat terms.
pub fn armour_line(definitions: &Definitions, item: ProductionItem) -> Option<String> {
    let terms = match item {
        ProductionItem::Unit(kind) => definitions.unit_combat(kind),
        ProductionItem::Building(kind) => definitions.building_combat(kind),
    }?;
    let name = definitions
        .armour_classes()
        .get(usize::from(terms.armour.0))?;
    Some(if name == "none" {
        "No armour".to_owned()
    } else {
        format!("{} armour", title(name))
    })
}

/// The name of the rank `rank` of a unit (RA4): `Veteran`, `Elite`, or `None` below them.
pub fn rank_name(rank: u8) -> Option<&'static str> {
    match rank {
        0 => None,
        1 => Some("Veteran"),
        _ => Some("Elite"),
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
    played: Option<Res<crate::skirmish::MatchPlayers>>,
    teams: Option<Res<crate::presentation::TeamColors>>,
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
        .filter(|_| {
            host.running()
                && !matches!(*mode, InterfaceMode::Place(_) | InterfaceMode::PlaceUnit(_))
        })
        .and_then(|cursor| {
            let definitions = host.definitions()?;
            let point = camera.viewport_to_world_2d(camera_transform, cursor).ok();
            let (entity, mut item, mut owner) = hovered_entity(&host, point, hovered.0, &views)?;
            let world = host.world();
            // The visible bar already conveys a health ratio, not an absolute strength. Keep
            // that same ratio while every identity-dependent field follows the rendered guise.
            let max = match item {
                ProductionItem::Unit(kind) => definitions.unit_combat(kind),
                ProductionItem::Building(kind) => definitions.building_combat(kind),
            }
            .map(|terms| terms.hit_points);
            let health = world
                .get::<Health>(entity)
                .map(|health| health.hit_points)
                .zip(max);
            let disguise = world.get::<SimId>(entity).and_then(|id| {
                world
                    .get_resource::<sim::SpecialState>()?
                    .disguise_for(HUMAN, *id)
            });
            if let Some(kind) = disguise {
                item = ProductionItem::Unit(kind);
                owner = HUMAN;
            }
            // Another player's are named by its team colour and the country it plays.
            let whose = (owner != HUMAN).then(|| {
                let teams = teams.as_deref().copied().unwrap_or_default();
                crate::skirmish::player_name(
                    crate::presentation::TEAM_PALETTE[teams.index_of(owner)].0,
                    played.as_deref().and_then(|played| played.label(owner)),
                )
            });
            let mut text = label_text(
                &display_name(definitions, item),
                owner,
                whose.as_deref(),
                health,
            );
            if let Some(armour) = armour_line(definitions, item) {
                text.push('\n');
                text.push_str(&armour);
            }
            // RA4: the rank of a veteran or elite unit.
            if disguise.is_none()
                && let Some(rank) = world
                    .get::<sim::Experience>(entity)
                    .and_then(|experience| rank_name(experience.rank))
            {
                text.push('\n');
                text.push_str(rank);
            }
            Some((
                text,
                if owner == sim::NEUTRAL {
                    Color::srgb(0.82, 0.82, 0.78)
                } else if owner == HUMAN {
                    Color::srgb(0.72, 0.9, 1.0)
                } else {
                    Color::srgb(1.0, 0.72, 0.66)
                },
                cursor,
            ))
        })
        .or_else(|| {
            // Unseen scenery uses one last-observed snapshot, including whether it was
            // occupied. Never consult live hidden Props/garrisons even to choose label presence.
            let cursor = cursor.filter(|_| host.running() && *mode == InterfaceMode::Normal)?;
            let cell = hovered.0?;
            let explored = host.players()?.get(HUMAN)?.vision().is_explored(cell);
            let observed = host.observed_scenery(cell).filter(|_| explored)?;
            let prop = observed.prop?;
            let mut text = scenery_text(prop.kind, prop.hit_points);
            if let Some((owner, count)) = observed.garrison {
                text.push_str(&format!(
                    "\n{} occupants · {}",
                    count,
                    if owner == HUMAN {
                        "Yours · D to evacuate"
                    } else {
                        "Enemy"
                    }
                ));
            }
            Some((text, Color::srgb(0.86, 0.84, 0.7), cursor))
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
pub(crate) fn privacy_scenery_label(app: &mut App, cell: sim::CellPos) -> (String, Display) {
    use bevy::ecs::system::RunSystemOnce;
    let has_label = app
        .world_mut()
        .query_filtered::<Entity, With<HoverLabel>>()
        .iter(app.world())
        .next()
        .is_some();
    if !has_label {
        app.world_mut().run_system_once(spawn_hover_label).unwrap();
    }
    crate::privacy_native::hover_cell(app, cell);
    app.world_mut()
        .run_system_once(show_hover_label)
        .expect("actual scenery hover");
    let world = app.world_mut();
    let mut query = world.query_filtered::<(&Text, &Node), With<HoverLabel>>();
    let (text, node) = query.single(world).unwrap();
    (text.0.clone(), node.display)
}

#[cfg(test)]
mod tests {
    use super::*;
    include!("privacy_hover_tests.rs");

    #[test]
    fn ra5_neutral_ownership_is_named_without_an_enemy_team_label() {
        assert_eq!(
            label_text(
                "Oil Derrick",
                sim::NEUTRAL,
                Some("Blue Player"),
                Some((1000, 1000))
            ),
            "Oil Derrick\nNeutral  100%"
        );
        assert_eq!(
            label_text("Oil Derrick", HUMAN, None, None),
            "Oil Derrick\nYours"
        );
    }

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
            label_text("Grizzly Battle Tank", HUMAN, None, Some((150, 300))),
            "Grizzly Battle Tank\nYours  50%"
        );
        assert_eq!(
            label_text("Rhino Heavy Tank", PlayerId(1), None, Some((1, 400))),
            "Rhino Heavy Tank\nEnemy  1%",
            "a living unit never reads 0%"
        );
        assert_eq!(label_text("Ore", PlayerId(2), None, None), "Ore\nEnemy");
        // RA1: another player's are named by its colour and the country it plays; the human
        // player's own stay "Yours" whatever name is given.
        let gold = crate::skirmish::player_name("Gold", Some("Iraq"));
        assert_eq!(gold, "Gold Player (Iraq)");
        assert_eq!(
            label_text(
                "Rhino Heavy Tank",
                PlayerId(2),
                Some(&gold),
                Some((200, 400))
            ),
            "Rhino Heavy Tank\nGold Player (Iraq)  50%"
        );
        assert_eq!(
            label_text("War Factory", PlayerId(1), Some("Red Player"), None),
            "War Factory\nRed Player"
        );
        assert_eq!(
            label_text("Grizzly Battle Tank", HUMAN, Some(&gold), Some((300, 300))),
            "Grizzly Battle Tank\nYours  100%"
        );
        assert_eq!(title("great_britain"), "Great Britain");
        assert_eq!(title("allied"), "Allied");
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

    /// RA3: the label names the armour type of what the cursor is on.
    /// RA4: the hover text names a veteran and an elite unit, and nothing below them.
    #[test]
    fn the_rank_of_a_unit_is_named_from_veteran_on() {
        assert_eq!(rank_name(0), None);
        assert_eq!(rank_name(sim::VETERAN), Some("Veteran"));
        assert_eq!(rank_name(sim::ELITE), Some("Elite"));
    }

    #[test]
    fn the_label_names_the_armour_of_every_shipped_type() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let (loaded, _) = crate::units::load_match_data(&root).expect("match data");
        let definitions = &loaded.definitions;
        let line = |id: &str| {
            let item = definitions
                .unit_kind(id)
                .map(ProductionItem::Unit)
                .or_else(|| definitions.building_kind(id).map(ProductionItem::Building))
                .unwrap_or_else(|| panic!("{id}"));
            armour_line(definitions, item).unwrap_or_else(|| panic!("{id} has no armour"))
        };
        assert_eq!(line("gi"), "No armour");
        assert_eq!(line("conscript"), "Flak armour");
        assert_eq!(line("tesla_trooper"), "Plate armour");
        assert_eq!(line("ifv"), "Light armour");
        assert_eq!(line("chrono_miner"), "Medium armour");
        assert_eq!(line("grizzly"), "Heavy armour");
        assert_eq!(line("power_plant"), "Wood armour");
        assert_eq!(line("pillbox"), "Steel armour");
        assert_eq!(line("construction_yard"), "Concrete armour");
        assert_eq!(line("terror_drone"), "Special 1 armour");
        let units =
            (0..definitions.units().len()).map(|i| ProductionItem::Unit(sim::UnitKind(i as u16)));
        let buildings = (0..definitions.buildings().len())
            .map(|i| ProductionItem::Building(sim::BuildingKind(i as u16)));
        for item in units.chain(buildings) {
            let text = armour_line(definitions, item).expect("every shipped type has armour");
            assert!(text.ends_with(" armour") && !text.contains('_'), "{text}");
        }
    }
}
