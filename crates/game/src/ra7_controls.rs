//! RA7 pure input decisions and camera/selection extensions. Simulation writes go through Sink.
use crate::{
    hotkeys::{Hotkey, pressed_hotkeys},
    map::{HoveredCell, MapView},
    selection::Selection,
    sim_host::{CommandSink, SimHost},
    units::HUMAN,
};
use bevy::prelude::*;
use sim::{CellPos, Command, ForceTarget, SimId};
use std::collections::BTreeSet;

pub struct Ra7ControlsPlugin;
#[derive(Resource, Default)]
struct Bookmarks {
    points: [Option<Vec2>; 4],
    match_tick: u64,
}
impl Plugin for Ra7ControlsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Bookmarks>().add_systems(
            PostUpdate,
            (extended_keys, psychic_lines).after(crate::selection::issue_orders),
        );
    }
}
/// Modifier precedence is shared by left/right clicks. Z queues moves while held.
pub fn modified_order(
    units: &BTreeSet<SimId>,
    cell: Option<CellPos>,
    target: Option<SimId>,
    control: bool,
    shift: bool,
    alt: bool,
    waypoint: bool,
) -> Option<Command> {
    if units.is_empty() {
        return None;
    }
    let cell = cell?;
    let units = units.iter().copied().collect();
    Some(if control && shift {
        Command::AttackMove {
            units,
            target: cell,
        }
    } else if control {
        Command::ForceFire {
            units,
            target: target.map_or(ForceTarget::Cell(cell), ForceTarget::Entity),
        }
    } else if alt {
        Command::ForceMove {
            units,
            target: cell,
        }
    } else if waypoint {
        Command::QueueMove {
            units,
            target: cell,
        }
    } else {
        return None;
    })
}
pub fn order_key(
    action: Hotkey,
    units: &BTreeSet<SimId>,
    target: Option<SimId>,
) -> Option<Command> {
    if units.is_empty() {
        return None;
    }
    let units = units.iter().copied().collect();
    match action {
        Hotkey::Guard => Some(Command::Guard { units }),
        Hotkey::Scatter => Some(Command::Scatter { units }),
        Hotkey::Follow => target.map(|target| Command::Follow { units, target }),
        _ => None,
    }
}
/// A known object under the cursor, with exact special concealment checks before returning IDs.
pub fn known_at(host: &SimHost, cell: Option<CellPos>) -> Option<SimId> {
    let cell = cell?;
    let state = host.players()?.get(HUMAN)?;
    let special = host.world().get_resource::<sim::SpecialState>()?;
    let grid = host.grid()?;
    let ids = grid
        .holders(cell)
        .into_iter()
        .flatten()
        .chain(
            grid.holders_for(cell, sim::Domain::Air)
                .into_iter()
                .flatten(),
        )
        .chain(grid.building(cell))
        .chain(host.props().and_then(|p| p.id_at(cell)));
    ids.filter(|id| !special.hidden.contains(&(HUMAN, *id)))
        .find(|id| {
            host.world()
                .iter_entities()
                .find(|e| e.get::<SimId>() == Some(id))
                .and_then(|e| Some((e.get::<sim::Owner>()?.0, e.get::<sim::Health>()?.hit_points)))
                .is_some_and(|(o, h)| h > 0 && (o == HUMAN || state.vision().is_visible(cell)))
                || host.props().is_some_and(|p| p.get(*id).is_some())
                    && state.vision().is_visible(cell)
        })
}
#[allow(clippy::too_many_arguments)]
fn extended_keys(
    keys: Res<ButtonInput<KeyCode>>,
    host: Res<SimHost>,
    map: Option<Res<MapView>>,
    hovered: Res<HoveredCell>,
    preview: Option<Res<crate::selection::OrderPreview>>,
    mut selection: ResMut<Selection>,
    groups: Res<crate::hotkeys::ControlGroups>,
    mut sink: ResMut<CommandSink>,
    mut bookmarks: ResMut<Bookmarks>,
    mut cameras: Query<&mut Transform, With<Camera2d>>,
) {
    if !host.running() {
        bookmarks.points = [None; 4];
        bookmarks.match_tick = 0;
        return;
    }
    if host.tick() < bookmarks.match_tick {
        bookmarks.points = [None; 4];
    }
    bookmarks.match_tick = host.tick();
    let Some(map) = map else {
        return;
    };
    let mut own: Vec<_> = host
        .world()
        .iter_entities()
        .filter_map(|e| {
            if e.get::<sim::Owner>()?.0 != HUMAN || e.get::<sim::Health>()?.hit_points <= 0 {
                return None;
            }
            let id = *e.get::<SimId>()?;
            if host
                .world()
                .resource::<sim::SpecialState>()
                .spawned
                .contains_key(&id)
                || host
                    .world()
                    .resource::<sim::SpecialState>()
                    .strategic
                    .flights
                    .contains_key(&id)
            {
                return None;
            }
            Some((
                id,
                e.get::<sim::Unit>()?.kind,
                e.get::<sim::Mobile>()?.cell,
                e.get::<sim::Weapon>().map(|w| w.kind),
            ))
        })
        .collect();
    own.sort_by_key(|u| u.0);
    for action in pressed_hotkeys(&keys) {
        sink.issue(order_key(
            action,
            &selection.0,
            preview
                .as_ref()
                .and_then(|p| p.target)
                .or_else(|| known_at(&host, hovered.0)),
        ));
        let mut centre = None;
        match action {
            Hotkey::SelectType => {
                if let Some(kind) = own.iter().find(|u| selection.0.contains(&u.0)).map(|u| u.1) {
                    selection.0 = own.iter().filter(|u| u.1 == kind).map(|u| u.0).collect();
                }
            }
            Hotkey::AllCombat => {
                selection.0 = own
                    .iter()
                    .filter(|u| {
                        u.3.and_then(|w| host.definitions()?.weapon(w))
                            .is_some_and(|w| {
                                w.damage > 0
                                    && !w.repair
                                    && host
                                        .definitions()
                                        .and_then(|d| d.warhead(w.warhead))
                                        .is_some_and(|h| {
                                            !h.makes_disguise
                                                && !h.bomb_disarm
                                                && !h.electric_assault
                                        })
                            })
                    })
                    .map(|u| u.0)
                    .collect()
            }
            Hotkey::NextUnit | Hotkey::PreviousUnit => {
                if !own.is_empty() {
                    let current = own.iter().position(|u| selection.0.contains(&u.0));
                    let n = if action == Hotkey::NextUnit {
                        current.map_or(0, |n| (n + 1) % own.len())
                    } else {
                        current.map_or(own.len() - 1, |n| (n + own.len() - 1) % own.len())
                    };
                    selection.0 = [own[n].0].into_iter().collect();
                    centre = Some(crate::projection::cell_centre(own[n].2, map.tile));
                }
            }
            Hotkey::Home => {
                let yards = crate::selection::own_yards(&host);
                centre = host
                    .world()
                    .iter_entities()
                    .find(|e| e.get::<SimId>().is_some_and(|id| yards.contains(id)))
                    .and_then(|e| e.get::<sim::Building>())
                    .map(|b| crate::projection::cell_centre(b.footprint.origin, map.tile));
            }
            Hotkey::CentreGroup(n) => {
                selection.0 = groups.0[usize::from(n).min(9)].clone();
                centre = own
                    .iter()
                    .find(|u| selection.0.contains(&u.0))
                    .map(|u| crate::projection::cell_centre(u.2, map.tile));
            }
            Hotkey::SetBookmark(n) => {
                if let Ok(c) = cameras.single() {
                    bookmarks.points[usize::from(n).min(3)] = Some(c.translation.truncate());
                }
            }
            Hotkey::JumpBookmark(n) => centre = bookmarks.points[usize::from(n).min(3)],
            _ => {}
        }
        if let Some(point) = centre
            && let Ok(mut c) = cameras.single_mut()
        {
            c.translation.x = point.x;
            c.translation.y = point.y;
        }
    }
}
fn psychic_lines(host: Res<SimHost>, map: Option<Res<MapView>>, mut gizmos: Gizmos) {
    let Some(map) = map else {
        return;
    };
    if !host.running() {
        return;
    }
    for order in sim::psychic_orders(host.world(), HUMAN) {
        gizmos.line_2d(
            crate::projection::cell_centre(order.from, map.tile),
            crate::projection::cell_centre(order.target, map.tile),
            Color::srgba(0.7, 0.25, 1.0, 0.65),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hotkeys::modified_hotkey;
    #[test]
    fn ra7_every_new_binding_maps_input_to_its_order_or_ui_action() {
        let units = [SimId(5), SimId(3)].into_iter().collect();
        let cell = CellPos::new(10, 12);
        let target = SimId(9);
        for (key, action, command) in [
            (
                KeyCode::KeyG,
                Hotkey::Guard,
                Command::Guard {
                    units: vec![SimId(3), SimId(5)],
                },
            ),
            (
                KeyCode::KeyX,
                Hotkey::Scatter,
                Command::Scatter {
                    units: vec![SimId(3), SimId(5)],
                },
            ),
            (
                KeyCode::KeyF,
                Hotkey::Follow,
                Command::Follow {
                    units: vec![SimId(3), SimId(5)],
                    target,
                },
            ),
        ] {
            assert_eq!(modified_hotkey(key, false, false, false), Some(action));
            assert_eq!(order_key(action, &units, Some(target)), Some(command));
            assert_eq!(order_key(action, &BTreeSet::new(), Some(target)), None);
        }
        for (key, action) in [
            (KeyCode::KeyQ, Hotkey::Tab(0)),
            (KeyCode::KeyW, Hotkey::Tab(1)),
            (KeyCode::KeyE, Hotkey::Tab(2)),
            (KeyCode::KeyR, Hotkey::Tab(3)),
            (KeyCode::KeyK, Hotkey::RepairMode),
            (KeyCode::KeyL, Hotkey::SellMode),
            (KeyCode::KeyZ, Hotkey::Waypoint),
            (KeyCode::KeyS, Hotkey::Stop),
            (KeyCode::KeyT, Hotkey::SelectType),
            (KeyCode::KeyP, Hotkey::AllCombat),
            (KeyCode::KeyH, Hotkey::Home),
            (KeyCode::KeyN, Hotkey::NextUnit),
            (KeyCode::KeyM, Hotkey::PreviousUnit),
            (KeyCode::KeyY, Hotkey::RallyAtCursor),
            (KeyCode::Pause, Hotkey::Pause),
            (KeyCode::F9, Hotkey::Pause),
            (KeyCode::Backslash, Hotkey::Pause),
            (KeyCode::F10, Hotkey::Mute),
            (KeyCode::Quote, Hotkey::Mute),
            (KeyCode::KeyI, Hotkey::Interact),
        ] {
            assert_eq!(
                modified_hotkey(key, false, false, false),
                Some(action),
                "{key:?}"
            );
        }
        assert_eq!(
            modified_order(&units, Some(cell), Some(target), true, true, true, true),
            Some(Command::AttackMove {
                units: vec![SimId(3), SimId(5)],
                target: cell
            })
        );
        assert_eq!(
            modified_order(&units, Some(cell), Some(target), true, false, true, true),
            Some(Command::ForceFire {
                units: vec![SimId(3), SimId(5)],
                target: ForceTarget::Entity(target)
            })
        );
        assert_eq!(
            modified_order(&units, Some(cell), None, true, false, false, false),
            Some(Command::ForceFire {
                units: vec![SimId(3), SimId(5)],
                target: ForceTarget::Cell(cell)
            })
        );
        assert_eq!(
            modified_order(&units, Some(cell), None, false, false, true, true),
            Some(Command::ForceMove {
                units: vec![SimId(3), SimId(5)],
                target: cell
            })
        );
        assert_eq!(
            modified_order(&units, Some(cell), None, false, false, false, true),
            Some(Command::QueueMove {
                units: vec![SimId(3), SimId(5)],
                target: cell
            })
        );
        assert_eq!(
            modified_order(&units, None, None, true, false, false, false),
            None
        );
        for (i, (key, arrow)) in [
            (KeyCode::F1, KeyCode::ArrowUp),
            (KeyCode::F2, KeyCode::ArrowRight),
            (KeyCode::F3, KeyCode::ArrowDown),
            (KeyCode::F4, KeyCode::ArrowLeft),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                modified_hotkey(key, true, false, false),
                Some(Hotkey::SetBookmark(i as u8))
            );
            assert_eq!(
                modified_hotkey(key, false, false, false),
                Some(Hotkey::JumpBookmark(i as u8))
            );
            assert_eq!(
                modified_hotkey(arrow, true, false, true),
                Some(Hotkey::SetBookmark(i as u8))
            );
            assert_eq!(
                modified_hotkey(arrow, false, false, true),
                Some(Hotkey::JumpBookmark(i as u8))
            );
        }
        assert_eq!(
            modified_hotkey(KeyCode::Digit3, false, false, true),
            Some(Hotkey::CentreGroup(3))
        );
    }
    #[test]
    fn ra7_selection_cycle_type_all_and_bookmark_actions_execute_on_client_state() {
        let host = crate::sim_host::tests::demo_host();
        let mut app = App::new();
        app.insert_resource(host)
            .init_resource::<Selection>()
            .init_resource::<crate::hotkeys::ControlGroups>()
            .init_resource::<CommandSink>()
            .init_resource::<Bookmarks>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<HoveredCell>()
            .insert_resource(MapView {
                width: 32,
                height: 32,
                tile: crate::projection::TileSize::new(64, 32),
            })
            .add_systems(Update, extended_keys);
        let camera = app
            .world_mut()
            .spawn((Camera2d, Transform::from_xyz(25.0, 50.0, 0.0)))
            .id();
        let press = |app: &mut App, key| {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(key);
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .reset_all();
        };
        press(&mut app, KeyCode::KeyN);
        let first = app.world().resource::<Selection>().0.clone();
        assert_eq!(first.len(), 1);
        press(&mut app, KeyCode::KeyN);
        assert_ne!(first, app.world().resource::<Selection>().0);
        press(&mut app, KeyCode::KeyM);
        assert_eq!(first, app.world().resource::<Selection>().0);
        press(&mut app, KeyCode::KeyT);
        assert!(!app.world().resource::<Selection>().0.is_empty());
        press(&mut app, KeyCode::KeyP);
        assert!(!app.world().resource::<Selection>().0.is_empty());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ControlLeft);
        press(&mut app, KeyCode::F1);
        let saved = app.world().resource::<Bookmarks>().points[0].unwrap();
        app.world_mut()
            .get_mut::<Transform>(camera)
            .unwrap()
            .translation
            .x += 100.0;
        press(&mut app, KeyCode::F1);
        assert_eq!(
            app.world()
                .get::<Transform>(camera)
                .unwrap()
                .translation
                .truncate(),
            saved
        );
    }
}

#[cfg(test)]
mod deployment_mapping {
    use super::*;
    #[test]
    fn ra7_deploy_key_maps_every_selected_unit_without_modifier_collisions() {
        let selection = Selection([SimId(1), SimId(2)].into_iter().collect());
        assert_eq!(
            crate::hotkeys::modified_hotkey(KeyCode::KeyD, false, false, false),
            Some(Hotkey::Deploy)
        );
        assert_eq!(
            crate::special_controls::deploy_commands(&selection),
            vec![
                Command::Deploy { unit: SimId(1) },
                Command::Deploy { unit: SimId(2) }
            ]
        );
        assert_eq!(
            crate::hotkeys::modified_hotkey(KeyCode::KeyD, true, false, false),
            None
        );
    }
}
