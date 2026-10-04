//! Interim economy controls (PLAN.md section 7, M3; docs/DECISIONS.md D27) until the sidebar of
//! M5: keys that queue and cancel production, place the ready building at the cursor, sell the
//! building under the cursor, and set rally points, and an outline of the ready building's
//! footprint at the cursor.
//!
//! | Input | Command |
//! |-------|---------|
//! | `1` to `9` | `QueueProduction` of the n-th entry of the production menu: the building types in the order of the economy file, then the producible unit types |
//! | Shift + `1` to `9` | `CancelProduction` of that entry |
//! | `B` | `PlaceBuilding` of the player's ready building, its footprint centred on the cell under the cursor |
//! | `Delete` or `Backspace` | `SellBuilding` of the player's building under the cursor |
//! | `R` | `SetRallyPoint` of every building of the player that produces units, to the cell under the cursor |
//!
//! As in `crate::selection`, the decisions are pure functions, unit tested below, and the
//! interface changes the simulation only through `CommandQueue` (invariant I5).

use std::collections::BTreeMap;

use bevy::camera::CameraUpdateSystems;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use sim::{
    Building, BuildingKind, CellPos, Command, CommandQueue, Definitions, Footprint, Grid,
    IssuedCommand, Owner, PlayerId, PlayerState, Players, ProductionItem, SimId,
};

use crate::buildings::own_building_at;
use crate::map::{HoveredCell, MapView};
use crate::projection::to_world;
use crate::units::HUMAN;

/// The keys of the production menu, in menu order.
pub const MENU_KEYS: [KeyCode; 9] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

pub struct ConstructionPlugin;

impl Plugin for ConstructionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (issue_economy_commands, draw_placement_outline)
                .chain()
                .after(TransformSystems::Propagate)
                .after(CameraUpdateSystems)
                .after(crate::map::update_hovered_cell)
                // Both push commands; the selection's orders come first in a frame.
                .after(crate::selection::issue_orders),
        );
    }
}

// --- Pure functions -------------------------------------------------------------------------

/// The production menu: every building type in the order of the economy file, then every unit
/// type with production terms in the order of the units file.
pub fn production_menu(definitions: &Definitions) -> Vec<ProductionItem> {
    let buildings = (0..definitions.buildings().len())
        .filter_map(|index| u16::try_from(index).ok())
        .map(|index| ProductionItem::Building(BuildingKind(index)));
    let units = (0..definitions.units().len())
        .filter_map(|index| u16::try_from(index).ok())
        .map(|index| ProductionItem::Unit(sim::UnitKind(index)))
        .filter(|&item| definitions.production(item).is_some());
    buildings.chain(units).collect()
}

/// The command for menu key `index` (0 for `1`): queue the entry, or cancel it with `cancel`
/// (Shift). Nothing for a key beyond the menu.
pub fn menu_command(menu: &[ProductionItem], index: usize, cancel: bool) -> Option<Command> {
    let item = *menu.get(index)?;
    Some(if cancel {
        Command::CancelProduction { item }
    } else {
        Command::QueueProduction { item }
    })
}

/// The building type of `state` that is complete and waits for placement: the head of a queue
/// whose progress has reached its build time (the first such queue in building type order).
pub fn ready_building(state: &PlayerState, definitions: &Definitions) -> Option<BuildingKind> {
    state.queues().find_map(|(_, items)| {
        let head = items.first()?;
        let ProductionItem::Building(kind) = head.item else {
            return None;
        };
        let terms = definitions.production(head.item)?;
        (head.progress >= terms.build_time).then_some(kind)
    })
}

/// The footprint of a building of `width` × `height` cells centred on `cell` (for an even side,
/// the cell is the one just above and left of the centre).
pub fn footprint_at(cell: CellPos, width: u8, height: u8) -> Footprint {
    Footprint {
        origin: CellPos::new(
            cell.x
                .saturating_sub(i32::from(width.saturating_sub(1) / 2)),
            cell.y
                .saturating_sub(i32::from(height.saturating_sub(1) / 2)),
        ),
        width,
        height,
    }
}

/// The command for the place key: place the ready building centred on the hovered cell.
/// Nothing without a ready building or outside the map.
pub fn place_command(
    ready: Option<BuildingKind>,
    hovered: Option<CellPos>,
    definitions: &Definitions,
) -> Option<Command> {
    let building = ready?;
    let def = definitions.building(building)?;
    let footprint = footprint_at(hovered?, def.width, def.height);
    Some(Command::PlaceBuilding {
        building,
        origin: footprint.origin,
    })
}

/// The command for the sell key: sell the player's building under the cursor, if any.
pub fn sell_command(
    grid: &Grid,
    owners: &BTreeMap<SimId, PlayerId>,
    player: PlayerId,
    hovered: Option<CellPos>,
) -> Option<Command> {
    let building = own_building_at(grid, owners, player, hovered?)?;
    Some(Command::SellBuilding { building })
}

/// The commands for the rally key: every building of `producers` (the player's buildings whose
/// type produces units) gets the hovered cell as its rally point.
pub fn rally_commands(producers: &[SimId], hovered: Option<CellPos>) -> Vec<Command> {
    let Some(cell) = hovered else {
        return Vec::new();
    };
    producers
        .iter()
        .map(|&building| Command::SetRallyPoint { building, cell })
        .collect()
}

// --- Systems --------------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn issue_economy_commands(
    keys: Res<ButtonInput<KeyCode>>,
    hovered: Res<HoveredCell>,
    definitions: Res<Definitions>,
    players: Res<Players>,
    grid: Res<Grid>,
    buildings: Query<(&SimId, &Owner, &Building)>,
    mut queue: ResMut<CommandQueue>,
) {
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let menu = production_menu(&definitions);
    let mut commands: Vec<Command> = MENU_KEYS
        .iter()
        .enumerate()
        .filter(|(_, key)| keys.just_pressed(**key))
        .filter_map(|(index, _)| menu_command(&menu, index, shift))
        .collect();
    if keys.just_pressed(KeyCode::KeyB) {
        let ready = players
            .get(HUMAN)
            .and_then(|state| ready_building(state, &definitions));
        commands.extend(place_command(ready, hovered.0, &definitions));
    }
    if keys.any_just_pressed([KeyCode::Delete, KeyCode::Backspace]) {
        let owners: BTreeMap<SimId, PlayerId> = buildings
            .iter()
            .map(|(id, owner, _)| (*id, owner.0))
            .collect();
        commands.extend(sell_command(&grid, &owners, HUMAN, hovered.0));
    }
    if keys.just_pressed(KeyCode::KeyR) {
        let mut producers: Vec<SimId> = buildings
            .iter()
            .filter(|(_, owner, building)| {
                owner.0 == HUMAN && definitions.produces_units(building.kind)
            })
            .map(|(id, ..)| *id)
            .collect();
        producers.sort_unstable();
        commands.extend(rally_commands(&producers, hovered.0));
    }
    for command in commands {
        queue.push(IssuedCommand {
            player: HUMAN,
            command,
        });
    }
}

/// Outlines the footprint the ready building would cover if placed at the cursor (validity
/// colouring arrives with the sidebar in M5).
fn draw_placement_outline(
    hovered: Res<HoveredCell>,
    view: Option<Res<MapView>>,
    definitions: Res<Definitions>,
    players: Res<Players>,
    mut gizmos: Gizmos,
) {
    let (Some(cell), Some(view)) = (hovered.0, view) else {
        return;
    };
    let Some(kind) = players
        .get(HUMAN)
        .and_then(|state| ready_building(state, &definitions))
    else {
        return;
    };
    let Some(def) = definitions.building(kind) else {
        return;
    };
    let footprint = footprint_at(cell, def.width, def.height);
    let (x, y) = (footprint.origin.x as f32, footprint.origin.y as f32);
    let (w, h) = (f32::from(def.width), f32::from(def.height));
    let corners = [
        to_world(x, y, view.tile),
        to_world(x + w, y, view.tile),
        to_world(x + w, y + h, view.tile),
        to_world(x, y + h, view.tile),
    ];
    gizmos.linestrip_2d(
        [corners[0], corners[1], corners[2], corners[3], corners[0]],
        Color::srgb(1.0, 0.9, 0.2),
    );
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn fixture_definitions() -> Definitions {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sim/tests/fixtures/data");
        sim::load_definitions(dir.join("units_economy.ron"), dir.join("economy.ron"))
            .unwrap_or_else(|error| panic!("{error}"))
    }

    #[test]
    fn menu_keys_queue_and_cancel_the_menu_entries() {
        let definitions = fixture_definitions();
        let menu = production_menu(&definitions);
        // Eight building types, then the walker and the harvester (the runner has no
        // production terms).
        assert_eq!(menu.len(), 10);
        assert_eq!(menu[0], ProductionItem::Building(BuildingKind(0)));
        assert_eq!(menu[8], ProductionItem::Unit(sim::UnitKind(0)));
        assert_eq!(menu[9], ProductionItem::Unit(sim::UnitKind(2)));
        assert_eq!(
            menu_command(&menu, 1, false),
            Some(Command::QueueProduction { item: menu[1] })
        );
        assert_eq!(
            menu_command(&menu, 1, true),
            Some(Command::CancelProduction { item: menu[1] })
        );
        assert_eq!(menu_command(&menu, 10, false), None);
        assert_eq!(menu_command(&[], 0, false), None);
    }

    #[test]
    fn the_ready_building_is_placed_centred_on_the_cursor() {
        let definitions = fixture_definitions();
        let power = definitions.building_kind("power").expect("power");
        let yard = definitions.building_kind("yard").expect("yard");
        // 2x2: the hovered cell is the top-left of the centre four.
        assert_eq!(
            place_command(Some(power), Some(CellPos::new(10, 10)), &definitions),
            Some(Command::PlaceBuilding {
                building: power,
                origin: CellPos::new(10, 10),
            })
        );
        // 3x3: the hovered cell is the centre.
        assert_eq!(
            place_command(Some(yard), Some(CellPos::new(10, 10)), &definitions),
            Some(Command::PlaceBuilding {
                building: yard,
                origin: CellPos::new(9, 9),
            })
        );
        assert_eq!(
            place_command(None, Some(CellPos::new(1, 1)), &definitions),
            None
        );
        assert_eq!(place_command(Some(power), None, &definitions), None);
        assert_eq!(
            footprint_at(CellPos::new(i32::MIN, 0), 8, 8).origin,
            CellPos::new(i32::MIN, -3)
        );
    }

    #[test]
    fn a_complete_building_at_the_head_of_a_queue_is_ready() {
        let definitions = fixture_definitions();
        let power = ProductionItem::Building(definitions.building_kind("power").expect("power"));
        // Through the simulation: queue a power plant and run its ten ticks.
        let mut app = App::new();
        app.add_plugins(sim::SimPlugin { seed: 0 });
        let map = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../sim/tests/fixtures/maps/economy_32x32.tmx");
        let grid = sim::load_map(map).expect("map").grid;
        app.world_mut().insert_resource(grid);
        app.world_mut().insert_resource(definitions.clone());
        app.world_mut()
            .resource_mut::<Players>()
            .add(HUMAN, 1_000)
            .expect("player");
        sim::spawn_building(app.world_mut(), HUMAN, "yard", CellPos::new(19, 6)).expect("yard");
        app.world_mut()
            .resource_mut::<CommandQueue>()
            .push(IssuedCommand {
                player: HUMAN,
                command: Command::QueueProduction { item: power },
            });
        let ready = |app: &App| {
            ready_building(
                app.world()
                    .resource::<Players>()
                    .get(HUMAN)
                    .expect("player"),
                &definitions,
            )
        };
        for _ in 0..9 {
            app.world_mut().run_schedule(sim::SimSchedule);
            assert_eq!(ready(&app), None);
        }
        app.world_mut().run_schedule(sim::SimSchedule);
        assert_eq!(ready(&app), definitions.building_kind("power"));
    }

    #[test]
    fn sell_and_rally_keys_name_the_players_buildings() {
        let definitions = fixture_definitions();
        let mut app = App::new();
        app.add_plugins(sim::SimPlugin { seed: 0 });
        let map = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../sim/tests/fixtures/maps/economy_32x32.tmx");
        app.world_mut()
            .insert_resource(sim::load_map(map).expect("map").grid);
        app.world_mut().insert_resource(definitions);
        let own = sim::spawn_building(app.world_mut(), HUMAN, "barracks", CellPos::new(5, 5))
            .expect("own");
        let other = PlayerId(1);
        let foreign = sim::spawn_building(app.world_mut(), other, "power", CellPos::new(9, 9))
            .expect("foreign");
        let owners = BTreeMap::from([(own, HUMAN), (foreign, other)]);
        let grid = app.world().resource::<Grid>().clone();
        assert_eq!(
            sell_command(&grid, &owners, HUMAN, Some(CellPos::new(6, 6))),
            Some(Command::SellBuilding { building: own })
        );
        assert_eq!(
            sell_command(&grid, &owners, HUMAN, Some(CellPos::new(9, 9))),
            None
        );
        assert_eq!(
            sell_command(&grid, &owners, HUMAN, Some(CellPos::new(1, 1))),
            None
        );
        assert_eq!(sell_command(&grid, &owners, HUMAN, None), None);
        assert_eq!(
            rally_commands(&[own], Some(CellPos::new(2, 3))),
            [Command::SetRallyPoint {
                building: own,
                cell: CellPos::new(2, 3),
            }]
        );
        assert_eq!(rally_commands(&[own], None), []);
        assert_eq!(rally_commands(&[], Some(CellPos::new(2, 3))), []);
    }
}
