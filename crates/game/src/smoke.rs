//! `--smoke-test <frames>`: run the real application for a fixed number of frames, then exit.
//!
//! Exits with code 0 only if no asset failed to load, the map was validated and spawned, all
//! configured starting units and buildings were spawned, and the commands that the smoke test
//! issues through `CommandQueue` were accepted and took effect: the move, which must have moved a
//! unit if at least [`MOVE_CHECK_TICKS`] ticks ran after it, and (M3) the production of the first
//! building the player can queue, which must have been charged for if at least
//! [`PRODUCTION_CHECK_TICKS`] ticks ran after it.

use std::collections::BTreeMap;

use bevy::asset::UntypedAssetLoadFailedEvent;
use bevy::prelude::*;
use sim::{
    Building, BuildingKind, CellPos, Command, CommandQueue, Definitions, IssuedCommand, Owner,
    Players, Position, ProductionItem, SimEvent, SimId, SimTick, SubPos,
};

use crate::buildings::{BuildingStatus, STARTING_BUILDINGS};
use crate::construction::production_menu;
use crate::map::MapStatus;
use crate::units::{HUMAN, UnitStatus, starting_unit_total};

/// The cell of the demo map that the smoke test orders the player's units to (grass, across the
/// open ground from their starting area).
pub const MOVE_TARGET: CellPos = CellPos::new(20, 26);

/// Ticks after the order from which some unit must have moved.
pub const MOVE_CHECK_TICKS: u64 = 40;

/// Ticks after the production order from which credits must have been charged for it.
pub const PRODUCTION_CHECK_TICKS: u64 = 2;

pub struct SmokeTestPlugin {
    pub frames: u32,
}

impl Plugin for SmokeTestPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SmokeTest {
            frames: self.frames,
            rendered: 0,
            order: None,
            production: None,
            rejected: false,
        })
        .add_systems(Update, (order_move, order_production, watch_events))
        .add_systems(Last, (fail_on_asset_load_error, count_frames).chain());
    }
}

#[derive(Resource, Debug)]
struct SmokeTest {
    frames: u32,
    rendered: u32,
    /// The tick at which the move was ordered, and the units' positions then.
    order: Option<(u64, BTreeMap<SimId, SubPos>)>,
    /// The tick at which production was ordered, and the item (`None` if the player could
    /// queue nothing).
    production: Option<(u64, Option<ProductionItem>)>,
    rejected: bool,
}

/// The first building of the production menu that `owned` (the player's building types) lets
/// the player queue: it has no prerequisites and the player owns a building of its producing
/// type.
pub fn first_queueable(
    definitions: &Definitions,
    owned: &[BuildingKind],
) -> Option<ProductionItem> {
    production_menu(definitions).into_iter().find(|&item| {
        matches!(item, ProductionItem::Building(_))
            && definitions.production(item).is_some_and(|terms| {
                terms.prerequisites.is_empty() && owned.contains(&terms.producer)
            })
    })
}

/// Queues the first building the player can produce once its starting buildings exist.
fn order_production(
    mut smoke: ResMut<SmokeTest>,
    status: Res<BuildingStatus>,
    tick: Res<SimTick>,
    definitions: Res<Definitions>,
    buildings: Query<(&Owner, &Building)>,
    mut queue: ResMut<CommandQueue>,
) {
    if smoke.production.is_some() || status.spawned == 0 {
        return;
    }
    let owned: Vec<BuildingKind> = buildings
        .iter()
        .filter(|(owner, _)| owner.0 == HUMAN)
        .map(|(_, building)| building.kind)
        .collect();
    if owned.is_empty() {
        // The buildings are spawned through the world directly; they appear in queries in the
        // same frame, but wait for them all the same.
        return;
    }
    let item = first_queueable(&definitions, &owned);
    if let Some(item) = item {
        queue.push(IssuedCommand {
            player: HUMAN,
            command: Command::QueueProduction { item },
        });
    }
    smoke.production = Some((tick.0, item));
}

/// Orders all of the player's units to [`MOVE_TARGET`] once they exist.
fn order_move(
    mut smoke: ResMut<SmokeTest>,
    status: Res<UnitStatus>,
    tick: Res<SimTick>,
    units: Query<(&SimId, &Owner, &Position)>,
    mut queue: ResMut<CommandQueue>,
) {
    if smoke.order.is_some() || status.spawned == 0 {
        return;
    }
    let start: BTreeMap<SimId, SubPos> = units
        .iter()
        .filter(|(_, owner, _)| owner.0 == HUMAN)
        .map(|(id, _, position)| (*id, position.0))
        .collect();
    queue.push(IssuedCommand {
        player: HUMAN,
        command: Command::Move {
            units: start.keys().copied().collect(),
            target: MOVE_TARGET,
        },
    });
    smoke.order = Some((tick.0, start));
}

fn watch_events(mut smoke: ResMut<SmokeTest>, mut events: MessageReader<SimEvent>) {
    for event in events.read() {
        if let SimEvent::CommandRejected { reason, .. } = event {
            error!("smoke test failed: a command of the smoke test was rejected: {reason}");
            smoke.rejected = true;
        }
    }
}

fn fail_on_asset_load_error(
    mut failures: MessageReader<UntypedAssetLoadFailedEvent>,
    mut exit: MessageWriter<AppExit>,
) {
    for failure in failures.read() {
        error!(
            "smoke test failed: asset {} failed to load: {}",
            failure.path, failure.error
        );
        exit.write(AppExit::error());
    }
}

#[allow(clippy::too_many_arguments)]
fn count_frames(
    mut smoke: ResMut<SmokeTest>,
    map: Res<MapStatus>,
    status: Res<UnitStatus>,
    buildings: Res<BuildingStatus>,
    players: Res<Players>,
    tick: Res<SimTick>,
    units: Query<(&SimId, &Position)>,
    mut exit: MessageWriter<AppExit>,
) {
    smoke.rendered += 1;
    if smoke.rendered < smoke.frames {
        return;
    }
    let (ticks_since_order, moved) = match &smoke.order {
        Some((ordered_at, start)) => (
            tick.0.saturating_sub(*ordered_at),
            units
                .iter()
                .filter(|(id, position)| start.get(id).is_some_and(|&at| at != position.0))
                .count(),
        ),
        None => (0, 0),
    };
    // Credits charged for everything in the player's queues.
    let charged: i64 = players.get(HUMAN).map_or(0, |state| {
        state
            .queues()
            .flat_map(|(_, items)| items.iter().map(|item| item.charged))
            .sum()
    });
    let (production_ordered, ticks_since_production) = match smoke.production {
        Some((at, item)) => (item.is_some(), tick.0.saturating_sub(at)),
        None => (false, 0),
    };
    info!(
        "smoke test: {} frames, {} simulation ticks, map created: {}, units spawned: {}, \
         units moved: {moved} in {ticks_since_order} ticks after the order, buildings spawned: \
         {}, production charged {charged} credits in {ticks_since_production} ticks",
        smoke.rendered, tick.0, map.map_created, status.spawned, buildings.spawned
    );
    let failures = smoke_failures(&SmokeOutcome {
        frames: smoke.frames,
        map_created: map.map_created,
        spawned: status.spawned,
        expected_units: starting_unit_total(),
        rejected: smoke.rejected,
        ticks_since_order,
        moved,
        buildings: buildings.spawned,
        expected_buildings: STARTING_BUILDINGS.len(),
        production_ordered: production_ordered || smoke.production.is_none(),
        ticks_since_production,
        charged,
    });
    if failures.is_empty() {
        exit.write(AppExit::Success);
    } else {
        for failure in failures {
            error!("smoke test failed: {failure}");
        }
        exit.write(AppExit::error());
    }
}

/// What a smoke run observed by its last frame.
#[derive(Debug, Clone, Copy)]
struct SmokeOutcome {
    frames: u32,
    map_created: bool,
    spawned: usize,
    /// The configured number of starting units.
    expected_units: usize,
    rejected: bool,
    ticks_since_order: u64,
    moved: usize,
    buildings: usize,
    /// The configured number of starting buildings.
    expected_buildings: usize,
    /// `false` if the player's starting buildings let it queue nothing.
    production_ordered: bool,
    ticks_since_production: u64,
    /// Credits charged so far for the player's queued items.
    charged: i64,
}

/// The reasons a smoke run fails; empty if it passes.
fn smoke_failures(outcome: &SmokeOutcome) -> Vec<String> {
    let mut failures = Vec::new();
    if !outcome.map_created {
        failures.push(format!(
            "the map was not created within {} frames",
            outcome.frames
        ));
    }
    if outcome.spawned != outcome.expected_units {
        failures.push(format!(
            "{} of {} starting units were spawned",
            outcome.spawned, outcome.expected_units
        ));
    }
    if outcome.buildings != outcome.expected_buildings {
        failures.push(format!(
            "{} of {} starting buildings were spawned",
            outcome.buildings, outcome.expected_buildings
        ));
    }
    if outcome.rejected {
        failures.push("a command of the smoke test was rejected".to_string());
    }
    if !outcome.production_ordered {
        failures.push("the player's starting buildings let it queue nothing".to_string());
    }
    if outcome.ticks_since_production >= PRODUCTION_CHECK_TICKS && outcome.charged == 0 {
        failures.push(format!(
            "nothing was charged for the queued production within {} ticks",
            outcome.ticks_since_production
        ));
    }
    if outcome.ticks_since_order >= MOVE_CHECK_TICKS && outcome.moved == 0 {
        failures.push(format!(
            "no unit moved within {} ticks of the order",
            outcome.ticks_since_order
        ));
    }
    failures
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASSING: SmokeOutcome = SmokeOutcome {
        frames: 600,
        map_created: true,
        spawned: 26,
        expected_units: 26,
        rejected: false,
        ticks_since_order: 198,
        moved: 18,
        buildings: 2,
        expected_buildings: 2,
        production_ordered: true,
        ticks_since_production: 198,
        charged: 396,
    };

    #[test]
    fn a_run_passes_only_with_every_configured_starting_unit() {
        assert_eq!(smoke_failures(&PASSING), Vec::<String>::new());
        // M2 review 1, P1-2: a map that holds only some of the starting units fails the run.
        for spawned in [0, 12, 25, 27] {
            let failures = smoke_failures(&SmokeOutcome { spawned, ..PASSING });
            assert_eq!(
                failures,
                vec![format!("{spawned} of 26 starting units were spawned")]
            );
        }
        assert_eq!(
            smoke_failures(&SmokeOutcome {
                map_created: false,
                rejected: true,
                moved: 0,
                ..PASSING
            })
            .len(),
            3
        );
        // Too few ticks after the order to require movement.
        assert!(
            smoke_failures(&SmokeOutcome {
                ticks_since_order: MOVE_CHECK_TICKS - 1,
                moved: 0,
                ..PASSING
            })
            .is_empty()
        );
    }

    #[test]
    fn a_run_passes_only_with_its_buildings_and_charged_production() {
        assert_eq!(
            smoke_failures(&SmokeOutcome {
                buildings: 1,
                ..PASSING
            }),
            ["1 of 2 starting buildings were spawned"]
        );
        assert_eq!(
            smoke_failures(&SmokeOutcome {
                production_ordered: false,
                ..PASSING
            }),
            ["the player's starting buildings let it queue nothing"]
        );
        assert_eq!(
            smoke_failures(&SmokeOutcome {
                charged: 0,
                ..PASSING
            }),
            ["nothing was charged for the queued production within 198 ticks"]
        );
        // Too few ticks after the order to require a charge.
        assert!(
            smoke_failures(&SmokeOutcome {
                ticks_since_production: PRODUCTION_CHECK_TICKS - 1,
                charged: 0,
                ..PASSING
            })
            .is_empty()
        );
    }

    #[test]
    fn the_demo_player_can_queue_a_building_with_its_starting_buildings() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let definitions = sim::load_definitions(
            root.join(crate::units::UNITS_PATH),
            root.join(crate::buildings::ECONOMY_PATH),
        )
        .unwrap_or_else(|error| panic!("{error}"));
        let owned: Vec<BuildingKind> = STARTING_BUILDINGS
            .iter()
            .filter(|(owner, ..)| *owner == HUMAN)
            .filter_map(|(_, building_type, _)| definitions.building_kind(building_type))
            .collect();
        let item = first_queueable(&definitions, &owned).expect("something to queue");
        let terms = definitions.production(item).expect("terms");
        assert!(terms.prerequisites.is_empty());
        // At least one credit is charged on the first tick, so the smoke check can see it.
        assert!(terms.cost / i64::from(terms.build_time) > 0 || terms.build_time == 1);
        assert_eq!(first_queueable(&definitions, &[]), None);
    }

    #[test]
    fn the_demo_match_expects_all_of_its_starting_units() {
        assert_eq!(starting_unit_total(), 12 + 6 + 5 + 3);
    }
}
