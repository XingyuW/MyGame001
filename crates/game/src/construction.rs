//! Construction on the map (PLAN.md section 7, M3 and M5; docs/DECISIONS.md D27, D35): the
//! interface modes in which a left click on the map places the ready building, sells a building,
//! or sets the rally point, the placement preview with validity colouring, and the hotkeys that
//! act at the cursor.
//!
//! | Input | Effect |
//! |-------|--------|
//! | Sidebar: a ready building, or `B` | placement mode: a preview of the building follows the cursor, green where `PlaceBuilding` would be accepted and red where it would be rejected; a left click on a green footprint places it (`PlaceBuilding`) |
//! | Sidebar: Sell, or `L` | sell mode: a left click on one of the player's buildings sells it (`SellBuilding`); the mode stays until it is left |
//! | Sidebar: Repair, or `K` | repair mode: a left click on one of the player's buildings starts repairing it, or stops (`RepairBuilding`); the mode stays until it is left |
//! | Sidebar: Rally | rally mode: a left click on a cell makes it the rally point of every unit-producing building of the player (`SetRallyPoint`) |
//! | Right click or `Escape` in a mode | leaves the mode |
//! | `Delete` or `Backspace` | `SellBuilding` of the player's building under the cursor |
//! | `Y` | `SetRallyPoint` of every unit-producing building of the player, to the cell under the cursor |
//!
//! The production queue itself is driven from the sidebar (`crate::sidebar`), which uses
//! [`production_menu`] and [`menu_command`]. As in `crate::selection`, the decisions are pure
//! functions, unit tested below, and the interface changes the simulation only through
//! `crate::sim_host::CommandSink` (invariant I5). The preview is computed with
//! `sim::check_placement`, the function the command's validation runs, so under fog of war it
//! reveals no more than a rejected command would (docs/DECISIONS.md D33).

use std::collections::BTreeMap;

use bevy::camera::CameraUpdateSystems;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use sim::{
    Building, BuildingKind, CellPos, Command, Definitions, Footprint, Grid, Owner, PlacementError,
    PlayerId, PlayerState, ProductionItem, RejectReason, SimId,
};

use crate::buildings::{BuildingSprites, BuildingView, own_building_at};
use crate::hotkeys::{Hotkey, pressed_hotkeys};
use crate::map::{HoveredCell, MapView};
use crate::projection::{cell_corners, to_world};
use crate::sim_host::{CommandSink, SimHost};
use crate::units::{HUMAN, UnitView};

pub struct ConstructionPlugin;

impl Plugin for ConstructionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InterfaceMode>()
            // After the exclusive loading of the catalogue, so that no Startup system runs
            // unordered beside it.
            .add_systems(
                Startup,
                spawn_placement_ghost.after(crate::skirmish::load_catalogue),
            )
            .add_systems(
                PostUpdate,
                (
                    issue_economy_commands,
                    draw_placement_preview,
                    update_placement_ghost,
                )
                    .chain()
                    .after(TransformSystems::Propagate)
                    .after(CameraUpdateSystems)
                    .after(crate::map::update_hovered_cell)
                    // Both push commands; the selection's orders come first in a frame.
                    .after(crate::selection::issue_orders),
            );
    }
}

/// What a left click on the map does.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InterfaceMode {
    /// Select units (`crate::selection`).
    #[default]
    Normal,
    Interact,
    Strategic(SimId),
    ChronoDestination(SimId, sim::CellPos),
    /// Place the ready building of this type.
    Place(BuildingKind),
    /// Sell the player's building under the cursor.
    Sell,
    /// Start or stop repairing the player's building under the cursor.
    Repair,
    /// Set the rally point of the player's unit-producing buildings.
    Rally,
}

/// The entity that shows the ready building at the cursor in placement mode.
#[derive(Component, Debug)]
pub struct PlacementGhost;

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
    let mut menu: Vec<ProductionItem> = buildings.chain(units).collect();
    // Within each tab, the cards follow the tech tree: what is buildable from the start first,
    // then what each completed structure unlocks (file order within a tier). Before this, the
    // Radar Tower that five Soviet-style units need sat on the last page of the Buildings tab.
    menu.sort_by_key(|&item| {
        (
            matches!(item, ProductionItem::Unit(_)),
            tech_tier(definitions, item),
        )
    });
    menu
}

/// How many structures deep `item` lies in the tech tree: 0 for what the construction yard
/// builds without prerequisites, otherwise one more than its deepest producer or prerequisite
/// (a building that produces itself does not count as its own requirement).
pub fn tech_tier(definitions: &Definitions, item: ProductionItem) -> u32 {
    fn tier(definitions: &Definitions, item: ProductionItem, depth: u32) -> u32 {
        let Some(terms) = definitions.production(item) else {
            return 0;
        };
        if depth > 32 {
            return depth;
        }
        std::iter::once(terms.producer)
            .chain(terms.prerequisites.iter().copied())
            .filter(|&kind| item != ProductionItem::Building(kind))
            .map(|kind| tier(definitions, ProductionItem::Building(kind), depth + 1) + 1)
            .max()
            .unwrap_or(0)
    }
    // The construction yard itself is the root every other tier counts from.
    tier(definitions, item, 0).saturating_sub(1)
}

/// The command for menu entry `index` (0 for the first): queue the entry, or cancel it with
/// `cancel`. Nothing for an index beyond the menu.
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

/// Every building type of `state` that is complete and waits for placement, in the order of the
/// queues (RA2: the two lanes of the Construction Yard may each hold one).
pub fn ready_buildings(state: &PlayerState, definitions: &Definitions) -> Vec<BuildingKind> {
    state
        .queues()
        .filter_map(|(_, items)| {
            let head = items.first()?;
            let ProductionItem::Building(kind) = head.item else {
                return None;
            };
            let terms = definitions.production(head.item)?;
            (head.progress >= terms.build_time).then_some(kind)
        })
        .collect()
}

/// The ready building of `state` the interface deals with in `mode`: the one that placement
/// mode is placing, if it is ready; otherwise the first ready one ([`ready_building`]).
pub fn ready_in_mode(
    mode: InterfaceMode,
    state: &PlayerState,
    definitions: &Definitions,
) -> Option<BuildingKind> {
    match mode {
        InterfaceMode::Place(kind) if ready_buildings(state, definitions).contains(&kind) => {
            Some(kind)
        }
        _ => ready_building(state, definitions),
    }
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

/// The command for placing the ready building centred on the hovered cell. Nothing without a
/// ready building or outside the map.
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

/// The command for selling the player's building under the cursor, if any.
pub fn sell_command(
    grid: &Grid,
    owners: &BTreeMap<SimId, PlayerId>,
    player: PlayerId,
    hovered: Option<CellPos>,
) -> Option<Command> {
    let building = own_building_at(grid, owners, player, hovered?)?;
    Some(Command::SellBuilding { building })
}

/// The command for starting or stopping the repair of the player's building under the cursor,
/// if any.
pub fn repair_command(
    grid: &Grid,
    owners: &BTreeMap<SimId, PlayerId>,
    player: PlayerId,
    hovered: Option<CellPos>,
) -> Option<Command> {
    let building = own_building_at(grid, owners, player, hovered?)?;
    Some(Command::RepairBuilding { building })
}

/// The commands for setting a rally point: every building of `producers` (the player's buildings
/// whose type produces units) gets the hovered cell as its rally point.
pub fn rally_commands(producers: &[SimId], hovered: Option<CellPos>) -> Vec<Command> {
    let Some(cell) = hovered else {
        return Vec::new();
    };
    producers
        .iter()
        .map(|&building| Command::SetRallyPoint { building, cell })
        .collect()
}

/// The mode after a hotkey: `B` enters placement mode for the ready building (`ready`), or stays
/// in the current mode if none is ready; `X` enters sell mode, or leaves it; `Escape` leaves any
/// mode.
pub fn mode_after_key(
    mode: InterfaceMode,
    key: Hotkey,
    ready: Option<BuildingKind>,
) -> InterfaceMode {
    match key {
        Hotkey::PlaceReady => ready.map_or(mode, InterfaceMode::Place),
        Hotkey::SellMode if mode == InterfaceMode::Sell => InterfaceMode::Normal,
        Hotkey::SellMode => InterfaceMode::Sell,
        Hotkey::RepairMode if mode == InterfaceMode::Repair => InterfaceMode::Normal,
        Hotkey::RepairMode => InterfaceMode::Repair,
        Hotkey::Cancel => InterfaceMode::Normal,
        _ => mode,
    }
}

/// Placement mode lasts while its building is ready; once it is placed or cancelled, the mode
/// ends.
pub fn mode_while_ready(mode: InterfaceMode, ready: Option<BuildingKind>) -> InterfaceMode {
    match mode {
        InterfaceMode::Place(kind) if ready != Some(kind) => InterfaceMode::Normal,
        mode => mode,
    }
}

/// What the map needs to know for a left click in a mode.
pub struct ClickContext<'a> {
    pub definitions: &'a Definitions,
    pub grid: &'a Grid,
    /// The owner of every building.
    pub owners: &'a BTreeMap<SimId, PlayerId>,
    /// The player's unit-producing buildings, in id order.
    pub producers: &'a [SimId],
    /// The outcome of `sim::check_placement` for the footprint at the cursor (placement mode).
    pub placement: Option<&'a Result<Footprint, RejectReason>>,
}

/// The commands of a left click on the map in `mode`, and the mode after it:
///
/// * placement: `PlaceBuilding` where the preview is valid, and the mode ends; a click on a red
///   footprint issues nothing and stays in the mode;
/// * sell: `SellBuilding` of the player's building under the cursor; the mode stays;
/// * repair: `RepairBuilding` of the player's building under the cursor; the mode stays;
/// * rally: `SetRallyPoint` of every unit-producing building; the mode ends;
/// * normal: nothing (clicks select units, `crate::selection`).
pub fn mode_click(
    mode: InterfaceMode,
    hovered: Option<CellPos>,
    context: &ClickContext,
) -> (Vec<Command>, InterfaceMode) {
    match mode {
        InterfaceMode::Normal
        | InterfaceMode::Interact
        | InterfaceMode::Strategic(_)
        | InterfaceMode::ChronoDestination(..) => (Vec::new(), mode),
        InterfaceMode::Place(kind) => match context.placement {
            Some(Ok(_)) => match place_command(Some(kind), hovered, context.definitions) {
                Some(command) => (vec![command], InterfaceMode::Normal),
                None => (Vec::new(), mode),
            },
            _ => (Vec::new(), mode),
        },
        InterfaceMode::Sell => (
            sell_command(context.grid, context.owners, HUMAN, hovered)
                .into_iter()
                .collect(),
            mode,
        ),
        InterfaceMode::Repair => (
            repair_command(context.grid, context.owners, HUMAN, hovered)
                .into_iter()
                .collect(),
            mode,
        ),
        InterfaceMode::Rally => match hovered {
            Some(_) => (
                rally_commands(context.producers, hovered),
                InterfaceMode::Normal,
            ),
            None => (Vec::new(), mode),
        },
    }
}

/// The cell a rejected placement names, to be marked in the preview: only cells the player sees
/// can be named, because `NotVisible` is checked first (D33).
pub fn problem_cell(reason: &RejectReason) -> Option<CellPos> {
    match reason {
        RejectReason::Placement(
            PlacementError::NotBuildable(cell)
            | PlacementError::NotVisible(cell)
            | PlacementError::Overlap { cell, .. }
            | PlacementError::Occupied { cell, .. },
        ) => Some(*cell),
        _ => None,
    }
}

/// Colour of the placement preview: green where the building may be placed, red where not.
pub fn preview_colour(valid: bool) -> Color {
    if valid {
        Color::srgba(0.3, 1.0, 0.3, 0.6)
    } else {
        Color::srgba(1.0, 0.25, 0.2, 0.6)
    }
}

// --- Reading the simulation -----------------------------------------------------------------

/// The owner of every building, and the human player's unit-producing buildings in id order.
fn building_owners(
    host: &SimHost,
    definitions: &Definitions,
) -> (BTreeMap<SimId, PlayerId>, Vec<SimId>) {
    let mut owners = BTreeMap::new();
    let mut producers = Vec::new();
    if let Some(mut buildings) = host
        .world()
        .try_query::<(&SimId, &Owner, &Building, Option<&sim::Health>)>()
    {
        for (id, owner, building, health) in buildings.iter(host.world()) {
            if health.is_some_and(|health| health.hit_points <= 0) {
                continue;
            }
            owners.insert(*id, owner.0);
            if owner.0 == HUMAN && definitions.produces_units(building.kind) {
                producers.push(*id);
            }
        }
    }
    producers.sort_unstable();
    (owners, producers)
}

/// The footprint of a building of type `kind` centred on `cell`, and whether `PlaceBuilding`
/// would accept it now: `sim::check_placement` with the human player's state and buildings.
pub fn placement_preview(
    host: &SimHost,
    kind: BuildingKind,
    cell: CellPos,
) -> Option<(Footprint, Result<Footprint, RejectReason>)> {
    let (grid, definitions) = (host.grid()?, host.definitions()?);
    let state = host.players()?.get(HUMAN)?;
    let def = definitions.building(kind)?;
    let footprint = footprint_at(cell, def.width, def.height);
    let mut owned = Vec::new();
    if let Some(mut buildings) = host
        .world()
        .try_query::<(&Owner, &Building, Option<&sim::Health>)>()
    {
        owned.extend(
            buildings
                .iter(host.world())
                .filter(|(owner, _, health)| {
                    owner.0 == HUMAN && !health.is_some_and(|health| health.hit_points <= 0)
                })
                .map(|(_, building, _)| (building.kind, building.footprint)),
        );
    }
    let check = sim::check_placement(
        grid,
        definitions,
        state,
        owned.into_iter(),
        kind,
        footprint.origin,
    );
    Some((footprint, check))
}

/// The human player's ready building that `mode` deals with ([`ready_in_mode`]).
fn human_ready_building(host: &SimHost, mode: InterfaceMode) -> Option<BuildingKind> {
    let definitions = host.definitions()?;
    ready_in_mode(mode, host.players()?.get(HUMAN)?, definitions)
}

// --- Systems --------------------------------------------------------------------------------

fn issue_economy_commands(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    hovered: Res<HoveredCell>,
    host: Res<SimHost>,
    mut mode: ResMut<InterfaceMode>,
    mut sink: ResMut<CommandSink>,
) {
    let Some(definitions) = host.definitions() else {
        return;
    };
    let ready = human_ready_building(&host, *mode);
    let mut next = mode_while_ready(*mode, ready);
    let mut commands = Vec::new();
    let (owners, producers) = building_owners(&host, definitions);
    for key in pressed_hotkeys(&keys) {
        match key {
            Hotkey::SellAtCursor => {
                if let Some(grid) = host.grid() {
                    commands.extend(sell_command(grid, &owners, HUMAN, hovered.0));
                }
            }
            Hotkey::RallyAtCursor => commands.extend(rally_commands(&producers, hovered.0)),
            Hotkey::PlaceReady if ready.is_none() => info!("no building is ready to be placed"),
            key => next = mode_after_key(next, key, ready),
        }
    }
    sink.issue(commands);
    if next != InterfaceMode::Normal && buttons.just_pressed(MouseButton::Right) {
        next = InterfaceMode::Normal;
    } else if buttons.just_pressed(MouseButton::Left)
        && hovered.0.is_some()
        && let Some(grid) = host.grid()
    {
        let placement = match (next, hovered.0) {
            (InterfaceMode::Place(kind), Some(cell)) => {
                placement_preview(&host, kind, cell).map(|(_, check)| check)
            }
            _ => None,
        };
        let context = ClickContext {
            definitions,
            grid,
            owners: &owners,
            producers: &producers,
            placement: placement.as_ref(),
        };
        let (click, after) = mode_click(next, hovered.0, &context);
        if let Some(Err(reason)) = &placement {
            info!("the building cannot be placed there: {reason}");
        }
        let expected = click.len();
        if sink.issue(click) == expected {
            next = after;
        }
    }
    if *mode != next {
        *mode = next;
    }
}

/// Outlines the footprint the ready building would cover at the cursor, green if
/// `PlaceBuilding` would be accepted there and red if not, with the cell a rejection names
/// marked.
fn draw_placement_preview(
    hovered: Res<HoveredCell>,
    view: Option<Res<MapView>>,
    mode: Res<InterfaceMode>,
    host: Res<SimHost>,
    mut gizmos: Gizmos,
) {
    let (Some(cell), Some(view), InterfaceMode::Place(kind)) = (hovered.0, view, *mode) else {
        return;
    };
    let Some((footprint, check)) = placement_preview(&host, kind, cell) else {
        return;
    };
    let colour = preview_colour(check.is_ok()).with_alpha(1.0);
    let (x, y) = (footprint.origin.x as f32, footprint.origin.y as f32);
    let (w, h) = (f32::from(footprint.width), f32::from(footprint.height));
    let corners = [
        to_world(x, y, view.tile),
        to_world(x + w, y, view.tile),
        to_world(x + w, y + h, view.tile),
        to_world(x, y + h, view.tile),
    ];
    gizmos.linestrip_2d(
        [corners[0], corners[1], corners[2], corners[3], corners[0]],
        colour,
    );
    for inner in footprint.cells() {
        let [top, right, bottom, left] = cell_corners(inner, view.tile);
        gizmos.linestrip_2d([top, right, bottom, left, top], colour.with_alpha(0.4));
    }
    if let Some(problem) = check.as_ref().err().and_then(problem_cell) {
        let [top, right, bottom, left] = cell_corners(problem, view.tile);
        gizmos.linestrip_2d([top, right, bottom, left, top], Color::srgb(1.0, 0.1, 0.1));
        gizmos.line_2d(top, bottom, Color::srgb(1.0, 0.1, 0.1));
        gizmos.line_2d(left, right, Color::srgb(1.0, 0.1, 0.1));
    }
}

fn spawn_placement_ghost(mut commands: Commands) {
    commands.spawn((
        PlacementGhost,
        Sprite::default(),
        Transform::default(),
        Visibility::Hidden,
    ));
}

/// Shows the ready building's sprite at the cursor in placement mode, tinted by validity.
#[allow(clippy::type_complexity)]
pub(crate) fn update_placement_ghost(
    hovered: Res<HoveredCell>,
    view: Option<Res<MapView>>,
    mode: Res<InterfaceMode>,
    host: Res<SimHost>,
    sprites: Option<Res<BuildingSprites>>,
    mut ghost: Query<
        (&mut Sprite, &mut Transform, &mut Visibility),
        (
            With<PlacementGhost>,
            Without<UnitView>,
            Without<BuildingView>,
        ),
    >,
) {
    for (mut sprite, mut transform, mut visibility) in &mut ghost {
        let shown = match (hovered.0, view.as_deref(), *mode, sprites.as_deref()) {
            (Some(cell), Some(view), InterfaceMode::Place(kind), Some(sprites)) => {
                placement_preview(&host, kind, cell).and_then(|(footprint, check)| {
                    let image = sprites.0.get(&kind)?.clone();
                    Some((footprint, check.is_ok(), image, view.tile))
                })
            }
            _ => None,
        };
        let Some((footprint, valid, image, tile)) = shown else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        };
        if sprite.image != image {
            sprite.image = image;
        }
        sprite.color = preview_colour(valid);
        // Above every unit and building (their depths lie below 2, `crate::units::unit_depth`).
        sprite.custom_size = Some(crate::hd_art::building_size(
            footprint.width,
            footprint.height,
        ));
        transform.translation = crate::hd_art::building_position(footprint, tile).extend(3.0);
        if *visibility != Visibility::Inherited {
            *visibility = Visibility::Inherited;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use sim::{CommandQueue, IssuedCommand, Players};

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
        // Repair, like sell, names only the player's own building under the cursor.
        assert_eq!(
            repair_command(&grid, &owners, HUMAN, Some(CellPos::new(6, 6))),
            Some(Command::RepairBuilding { building: own })
        );
        assert_eq!(
            repair_command(&grid, &owners, HUMAN, Some(CellPos::new(9, 9))),
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

    #[test]
    fn every_command_variant_is_built_by_a_pure_interface_function() {
        // PLAN.md section 7, M5: the functions that map interface actions to `Command` values
        // are pure and unit tested for every command variant. Each is called here as the
        // interface calls it; the `match` below has no wildcard, so a new variant fails to
        // compile until it is mapped.
        use std::collections::BTreeSet;

        use crate::selection::{
            attack_commands, attack_move_command, right_click_commands, stop_command,
        };
        use crate::sidebar::{ItemStatus, SidebarButton, SidebarEffect, button_effect};

        let definitions = fixture_definitions();
        let mut app = App::new();
        app.add_plugins(sim::SimPlugin { seed: 0 });
        let map = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../sim/tests/fixtures/maps/economy_32x32.tmx");
        app.world_mut()
            .insert_resource(sim::load_map(map).expect("map").grid);
        app.world_mut().insert_resource(definitions.clone());
        let barracks = sim::spawn_building(app.world_mut(), HUMAN, "barracks", CellPos::new(5, 5))
            .expect("barracks");
        let grid = app.world().resource::<Grid>().clone();
        let owners = BTreeMap::from([(barracks, HUMAN)]);
        let menu = production_menu(&definitions);
        let power = definitions.building_kind("power").expect("power");
        let item = ProductionItem::Building(power);
        let selection: BTreeSet<SimId> = [SimId(1), SimId(2)].into();
        let cell = CellPos::new(12, 12);

        let mut commands = Vec::new();
        // Map orders (`crate::selection`): a right click on ore with a harvester selected, a
        // right click on an enemy with an armed unit selected, `A`, and `S`.
        commands.extend(right_click_commands(
            &selection,
            &[SimId(2)].into(),
            Some(cell),
            true,
        ));
        commands.extend(attack_commands(
            &selection,
            &[SimId(1)].into(),
            SimId(9),
            Some(cell),
        ));
        commands.extend(attack_move_command(&selection, Some(cell)));
        commands.extend(stop_command(&selection));
        // Production (`crate::sidebar`): left and right clicks on an item.
        for (mouse, status) in [
            (
                MouseButton::Left,
                ItemStatus {
                    available: true,
                    ..ItemStatus::default()
                },
            ),
            (
                MouseButton::Right,
                ItemStatus {
                    available: true,
                    queued: 1,
                    ..ItemStatus::default()
                },
            ),
        ] {
            match button_effect(
                SidebarButton::Item(item),
                mouse,
                &menu,
                status,
                InterfaceMode::Normal,
            ) {
                SidebarEffect::Commands(issued) => commands.extend(issued),
                effect => panic!("{effect:?}"),
            }
        }
        // Map clicks in the modes of this module.
        let footprint = footprint_at(cell, 2, 2);
        let valid = Ok(footprint);
        let producers = [barracks];
        let context = |placement| ClickContext {
            definitions: &definitions,
            grid: &grid,
            owners: &owners,
            producers: &producers,
            placement,
        };
        let (placed, after) = mode_click(
            InterfaceMode::Place(power),
            Some(cell),
            &context(Some(&valid)),
        );
        assert_eq!(after, InterfaceMode::Normal);
        commands.extend(placed);
        let (sold, after) = mode_click(
            InterfaceMode::Sell,
            Some(CellPos::new(6, 6)),
            &context(None),
        );
        assert_eq!(after, InterfaceMode::Sell);
        commands.extend(sold);
        let (repaired, after) = mode_click(
            InterfaceMode::Repair,
            Some(CellPos::new(6, 6)),
            &context(None),
        );
        assert_eq!(after, InterfaceMode::Repair);
        commands.extend(repaired);
        let (rallied, after) = mode_click(InterfaceMode::Rally, Some(cell), &context(None));
        assert_eq!(after, InterfaceMode::Normal);
        commands.extend(rallied);

        let selected = crate::selection::Selection(selection.clone());
        commands.extend(crate::special_controls::deploy_commands(&selected));
        commands.extend(crate::special_controls::interact_commands(
            &selected, barracks,
        ));
        commands.push(crate::special_controls::undeploy_command(barracks));
        commands.push(crate::special_controls::strategic_command(
            barracks, cell, &selected,
        ));
        let mut variants = BTreeSet::new();
        for command in &commands {
            variants.insert(match command {
                Command::Deploy { .. } => "Deploy",
                Command::Undeploy { .. } => "Undeploy",
                Command::Interact { .. } => "Interact",
                Command::Superweapon { .. } | Command::Chronoshift { .. } => "Superweapon",
                Command::Move { .. } => "Move",
                Command::Stop { .. } => "Stop",
                Command::Harvest { .. } => "Harvest",
                Command::QueueProduction { .. } => "QueueProduction",
                Command::CancelProduction { .. } => "CancelProduction",
                Command::PlaceBuilding { .. } => "PlaceBuilding",
                Command::SellBuilding { .. } => "SellBuilding",
                Command::RepairBuilding { .. } => "RepairBuilding",
                Command::SetRallyPoint { .. } => "SetRallyPoint",
                Command::Attack { .. } => "Attack",
                Command::AttackMove { .. } => "AttackMove",
                Command::Guard { .. } => "Guard",
                Command::Scatter { .. } => "Scatter",
                Command::Follow { .. } => "Follow",
                Command::ForceFire { .. } => "ForceFire",
                Command::ForceMove { .. } => "ForceMove",
                Command::QueueMove { .. } => "QueueMove",
            });
        }
        assert_eq!(variants.len(), 15, "{variants:?}");
        assert!(commands.contains(&Command::PlaceBuilding {
            building: power,
            origin: footprint.origin,
        }));
        assert!(commands.contains(&Command::SellBuilding { building: barracks }));
        assert!(commands.contains(&Command::RepairBuilding { building: barracks }));
        assert!(commands.contains(&Command::SetRallyPoint {
            building: barracks,
            cell,
        }));
    }

    #[test]
    fn two_ready_buildings_of_the_two_lanes_are_each_placed_in_their_own_mode() {
        // RA2: a Power Plant in the lane of the buildings and a Pillbox in the lane of the
        // defences (it needs a Barracks) are built side by side and are ready together.
        // (The second Power Plant feeds the Barracks, so that nothing is slowed down.)
        let mut host = crate::sim_host::tests::demo_host_with(|world| {
            for id in ["power_plant", "barracks"] {
                let spot = (30..60)
                    .flat_map(|y| (20..60).map(move |x| sim::CellPos::new(x, y)))
                    .find(|&cell| sim::spawn_building(world, HUMAN, id, cell).is_ok());
                assert!(spot.is_some(), "ground for {id}");
            }
        });
        let definitions = host.definitions().expect("definitions").clone();
        let kind = |id: &str| definitions.building_kind(id).expect("building");
        let (power, pillbox) = (kind("power_plant"), kind("pillbox"));
        let time = |kind| {
            definitions
                .production(ProductionItem::Building(kind))
                .expect("terms")
                .build_time
        };
        crate::sim_host::tests::run(
            &mut host,
            [power, pillbox]
                .map(|kind| Command::QueueProduction {
                    item: ProductionItem::Building(kind),
                })
                .to_vec(),
        );
        for _ in 1..time(power).max(time(pillbox)) {
            crate::sim_host::tests::run(&mut host, Vec::new());
        }
        let state = host.players().and_then(|p| p.get(HUMAN)).expect("human");
        assert_eq!(ready_buildings(state, &definitions), [power, pillbox]);
        assert_eq!(ready_building(state, &definitions), Some(power));
        // The mode keeps the building it is placing; any other mode deals with the first.
        let place = InterfaceMode::Place(pillbox);
        assert_eq!(ready_in_mode(place, state, &definitions), Some(pillbox));
        assert_eq!(
            mode_while_ready(place, ready_in_mode(place, state, &definitions)),
            place
        );
        assert_eq!(
            ready_in_mode(InterfaceMode::Normal, state, &definitions),
            Some(power)
        );
        // A building that is not ready cannot be the one in placement.
        let barracks = InterfaceMode::Place(kind("barracks"));
        assert_eq!(
            mode_while_ready(barracks, ready_in_mode(barracks, state, &definitions)),
            InterfaceMode::Normal
        );
    }

    #[test]
    fn modes_follow_keys_clicks_and_the_ready_building() {
        let definitions = fixture_definitions();
        let power = definitions.building_kind("power").expect("power");
        let yard = definitions.building_kind("yard").expect("yard");
        // `B` enters placement mode only with a ready building; `X` toggles sell mode; `Escape`
        // leaves any mode; other keys change nothing.
        assert_eq!(
            mode_after_key(InterfaceMode::Normal, Hotkey::PlaceReady, Some(power)),
            InterfaceMode::Place(power)
        );
        assert_eq!(
            mode_after_key(InterfaceMode::Sell, Hotkey::PlaceReady, None),
            InterfaceMode::Sell
        );
        assert_eq!(
            mode_after_key(InterfaceMode::Normal, Hotkey::SellMode, None),
            InterfaceMode::Sell
        );
        assert_eq!(
            mode_after_key(InterfaceMode::Sell, Hotkey::SellMode, None),
            InterfaceMode::Normal
        );
        // `K` toggles repair mode, from any mode.
        assert_eq!(
            mode_after_key(InterfaceMode::Sell, Hotkey::RepairMode, None),
            InterfaceMode::Repair
        );
        assert_eq!(
            mode_after_key(InterfaceMode::Repair, Hotkey::RepairMode, None),
            InterfaceMode::Normal
        );
        for mode in [
            InterfaceMode::Place(power),
            InterfaceMode::Sell,
            InterfaceMode::Rally,
        ] {
            assert_eq!(
                mode_after_key(mode, Hotkey::Cancel, None),
                InterfaceMode::Normal
            );
            assert_eq!(mode_after_key(mode, Hotkey::Stop, None), mode);
        }
        // Placement mode ends when its building is no longer the ready one.
        assert_eq!(
            mode_while_ready(InterfaceMode::Place(power), Some(power)),
            InterfaceMode::Place(power)
        );
        assert_eq!(
            mode_while_ready(InterfaceMode::Place(power), Some(yard)),
            InterfaceMode::Normal
        );
        assert_eq!(
            mode_while_ready(InterfaceMode::Place(power), None),
            InterfaceMode::Normal
        );
        assert_eq!(
            mode_while_ready(InterfaceMode::Sell, None),
            InterfaceMode::Sell
        );
        // A click on a red footprint, or outside the map, issues nothing and keeps the mode.
        let grid = Grid::default();
        let owners = BTreeMap::new();
        let rejected = Err(RejectReason::Placement(PlacementError::NotBuildable(
            CellPos::new(1, 1),
        )));
        let producers: Vec<SimId> = Vec::new();
        let context = |placement| ClickContext {
            definitions: &definitions,
            grid: &grid,
            owners: &owners,
            producers: &producers,
            placement,
        };
        assert_eq!(
            mode_click(
                InterfaceMode::Place(power),
                Some(CellPos::new(1, 1)),
                &context(Some(&rejected))
            ),
            (Vec::new(), InterfaceMode::Place(power))
        );
        assert_eq!(
            mode_click(
                InterfaceMode::Place(power),
                Some(CellPos::new(1, 1)),
                &context(None)
            ),
            (Vec::new(), InterfaceMode::Place(power))
        );
        assert_eq!(
            mode_click(InterfaceMode::Rally, None, &context(None)),
            (Vec::new(), InterfaceMode::Rally)
        );
        assert_eq!(
            mode_click(
                InterfaceMode::Normal,
                Some(CellPos::new(1, 1)),
                &context(None)
            ),
            (Vec::new(), InterfaceMode::Normal)
        );
        // The cell a rejection names is marked; reasons without a cell mark none.
        assert_eq!(
            problem_cell(&rejected.clone().unwrap_err()),
            Some(CellPos::new(1, 1))
        );
        assert_eq!(problem_cell(&RejectReason::NotReady(power)), None);
        assert_ne!(preview_colour(true), preview_colour(false));
    }

    #[test]
    fn the_placement_preview_is_the_commands_validation_for_the_human_player() {
        // Through the simulation host: the demo match, before anything is ready.
        let host = crate::sim_host::tests::demo_host();
        let definitions = host.definitions().expect("definitions");
        let power = definitions
            .building_kind("power_plant")
            .expect("power plant");
        let (footprint, check) =
            placement_preview(&host, power, CellPos::new(19, 38)).expect("preview");
        assert_eq!(footprint.origin, CellPos::new(19, 38));
        assert_eq!(check, Err(RejectReason::NotReady(power)));
    }

    // Append inside construction::tests. Requires Tree corrected test-only take_commands helper.
    fn ra9_ready_boundary_host() -> (SimHost, BuildingKind, CellPos) {
        let mut host = crate::sim_host::tests::fixture_host(|world| {
            world
                .resource_mut::<sim::Players>()
                .add(HUMAN, 10_000)
                .expect("player");
            for (kind, cell) in [
                ("yard", CellPos::new(5, 5)),
                ("power", CellPos::new(9, 5)),
                ("barracks", CellPos::new(5, 10)),
                ("factory", CellPos::new(10, 10)),
            ] {
                sim::spawn_building(world, HUMAN, kind, cell).expect("scenario setup");
            }
        });
        let kind = host
            .definitions()
            .expect("definitions")
            .building_kind("power")
            .expect("power");
        let item = ProductionItem::Building(kind);
        let budget = host
            .definitions()
            .expect("definitions")
            .production(item)
            .expect("terms")
            .build_time
            * 4;
        let events =
            crate::sim_host::tests::run(&mut host, vec![Command::QueueProduction { item }]);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, sim::SimEvent::CommandRejected { .. }))
        );
        for _ in 0..budget {
            if human_ready_building(&host, InterfaceMode::Place(kind)) == Some(kind) {
                break;
            }
            crate::sim_host::tests::run(&mut host, vec![]);
        }
        assert_eq!(
            human_ready_building(&host, InterfaceMode::Place(kind)),
            Some(kind)
        );
        let cell = (0..32)
            .flat_map(|y| (0..32).map(move |x| CellPos::new(x, y)))
            .find(|&cell| placement_preview(&host, kind, cell).is_some_and(|(_, r)| r.is_ok()))
            .expect("actual valid preview");
        (host, kind, cell)
    }

    fn ra9_construction_app(host: SimHost, mode: InterfaceMode, cell: Option<CellPos>) -> App {
        let mut app = App::new();
        app.insert_resource(host)
            .init_resource::<CommandSink>()
            .insert_resource(mode)
            .insert_resource(HoveredCell(cell))
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, issue_economy_commands);
        app
    }

    #[test]
    fn ra9_scheduled_economy_click_transitions_require_actual_sink_acceptance_and_execute() {
        for placing in [false, true] {
            for refusal in ["none", "paused", "full", "closed"] {
                let (host, kind, cell) = ra9_ready_boundary_host();
                let producers = building_owners(&host, host.definitions().expect("definitions")).1;
                assert!(producers.len() >= 2, "real multi-producer rally setup");
                let mode = if placing {
                    InterfaceMode::Place(kind)
                } else {
                    InterfaceMode::Rally
                };
                let mut app = ra9_construction_app(host, mode, Some(cell));
                if refusal == "paused" {
                    app.world_mut().resource_mut::<CommandSink>().toggle_pause();
                }
                if refusal == "full" {
                    for _ in 0..crate::sim_host::MAX_PENDING_COMMANDS {
                        app.world_mut()
                            .resource_mut::<CommandSink>()
                            .push(Command::SetRallyPoint {
                                building: producers[0],
                                cell,
                            })
                            .expect("fill actual sink");
                    }
                }
                if refusal == "closed" {
                    let host = app
                        .world_mut()
                        .remove_resource::<SimHost>()
                        .expect("scenario host");
                    crate::sim_host::uninstall_match(app.world_mut()); // Real close path.
                    app.world_mut().insert_resource(host); // Definitions retained only for this closed-sink scenario.
                }
                let before = app.world().resource::<CommandSink>().accepted();
                app.world_mut()
                    .resource_mut::<ButtonInput<MouseButton>>()
                    .press(MouseButton::Left);
                app.world_mut().run_schedule(Update);
                let sink = app.world().resource::<CommandSink>();
                let expected = if placing { 1 } else { producers.len() as u64 };
                assert_eq!(
                    sink.accepted() - before,
                    if refusal == "none" { expected } else { 0 }
                );
                assert_eq!(
                    *app.world().resource::<InterfaceMode>(),
                    if refusal == "none" {
                        InterfaceMode::Normal
                    } else {
                        mode
                    }
                );
                if refusal != "none" {
                    assert!(sink.refused() > 0);
                    continue;
                }
                let commands = crate::sim_host::tests::take_commands(&mut app);
                assert_eq!(
                    commands.len(),
                    expected as usize,
                    "actual emitted batch, not just counters"
                );
                let mut host = app.world_mut().remove_resource::<SimHost>().expect("host");
                let events = crate::sim_host::tests::run(&mut host, commands);
                assert!(
                    !events
                        .iter()
                        .any(|e| matches!(e, sim::SimEvent::CommandRejected { .. })),
                    "{events:?}"
                );
                if placing {
                    assert!(
                        events.iter().any(
                            |e| matches!(e,sim::SimEvent::BuildingPlaced{kind:k,..}if *k==kind)
                        )
                    );
                } else {
                    let mut query = host
                        .world()
                        .try_query::<(&sim::SimId, &sim::Producer)>()
                        .expect("producers");
                    assert!(
                        query
                            .iter(host.world())
                            .filter(|(id, _)| producers.contains(id))
                            .all(|(_, producer)| producer.rally == Some(cell))
                    );
                }
            }
        }
    }

    #[test]
    fn ra9_partial_rally_accepts_only_available_slots_and_keeps_rally_mode() {
        let (host, _, cell) = ra9_ready_boundary_host();
        let producers = building_owners(&host, host.definitions().expect("definitions")).1;
        assert_eq!(producers.len(), 2, "two genuine producers");
        let old = CellPos::new(7, 8);
        let mut app = ra9_construction_app(host, InterfaceMode::Rally, Some(cell));
        for _ in 0..crate::sim_host::MAX_PENDING_COMMANDS - 1 {
            app.world_mut()
                .resource_mut::<CommandSink>()
                .push(Command::SetRallyPoint {
                    building: producers[0],
                    cell: old,
                })
                .expect("fill");
        }
        let before = app.world().resource::<CommandSink>().accepted();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.world_mut().run_schedule(Update);
        assert_eq!(
            *app.world().resource::<InterfaceMode>(),
            InterfaceMode::Rally
        );
        let sink = app.world().resource::<CommandSink>();
        assert_eq!(sink.accepted() - before, 1);
        assert_eq!(sink.refused(), 1);
        let commands = crate::sim_host::tests::take_commands(&mut app);
        let mut host = app.world_mut().remove_resource::<SimHost>().expect("host");
        let events = crate::sim_host::tests::run(&mut host, commands);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, sim::SimEvent::CommandRejected { .. })),
            "{events:?}"
        );
        let mut query = host
            .world()
            .try_query::<(&sim::SimId, &sim::Producer)>()
            .expect("producers");
        for (id, producer) in query.iter(host.world()) {
            if *id == producers[0] {
                assert_eq!(producer.rally, Some(cell));
            }
            if *id == producers[1] {
                assert_eq!(producer.rally, None);
            }
        }
    }

    #[test]
    fn ra9_cancel_invalid_preview_and_no_longer_ready_transitions_need_no_sink_slot() {
        for paused in [false, true] {
            for escape in [false, true] {
                let (host, kind, cell) = ra9_ready_boundary_host();
                let mut app = ra9_construction_app(host, InterfaceMode::Place(kind), Some(cell));
                if paused {
                    app.world_mut().resource_mut::<CommandSink>().toggle_pause();
                } else {
                    for _ in 0..crate::sim_host::MAX_PENDING_COMMANDS {
                        app.world_mut()
                            .resource_mut::<CommandSink>()
                            .push(Command::Stop { units: vec![] })
                            .expect("fill only; never executed");
                    }
                }
                let before = app.world().resource::<CommandSink>().accepted();
                if escape {
                    app.world_mut()
                        .resource_mut::<ButtonInput<KeyCode>>()
                        .press(KeyCode::Escape);
                } else {
                    app.world_mut()
                        .resource_mut::<ButtonInput<MouseButton>>()
                        .press(MouseButton::Right);
                }
                app.world_mut().run_schedule(Update);
                assert_eq!(
                    *app.world().resource::<InterfaceMode>(),
                    InterfaceMode::Normal
                );
                let sink = app.world().resource::<CommandSink>();
                assert_eq!(sink.accepted(), before);
                assert_eq!(sink.refused(), 0);
            }
        }
        for hovered in [None, Some(CellPos::new(-1, -1)), Some(CellPos::new(5, 5))] {
            let (host, kind, _) = ra9_ready_boundary_host();
            if let Some(cell) = hovered {
                assert!(placement_preview(&host, kind, cell).is_none_or(|(_, r)| r.is_err()));
            }
            let mut app = ra9_construction_app(host, InterfaceMode::Place(kind), hovered);
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Left);
            app.world_mut().run_schedule(Update);
            assert_eq!(
                *app.world().resource::<InterfaceMode>(),
                InterfaceMode::Place(kind)
            );
            let sink = app.world().resource::<CommandSink>();
            assert_eq!((sink.accepted(), sink.refused()), (0, 0));
        }
        let (mut host, kind, cell) = ra9_ready_boundary_host();
        let events = crate::sim_host::tests::run(
            &mut host,
            vec![Command::CancelProduction {
                item: ProductionItem::Building(kind),
            }],
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, sim::SimEvent::ProductionCancelled { .. }))
        );
        let mut app = ra9_construction_app(host, InterfaceMode::Place(kind), Some(cell));
        app.world_mut().run_schedule(Update);
        assert_eq!(
            *app.world().resource::<InterfaceMode>(),
            InterfaceMode::Normal
        );
        assert_eq!(app.world().resource::<CommandSink>().accepted(), 0);
    }
}
