//! `--smoke-test <frames>`: run the real application for a fixed number of frames, then exit.
//!
//! Exits with code 0 only if no asset failed to load, the map was validated and spawned, all
//! configured starting units were spawned, and the move that the smoke test orders through
//! `CommandQueue` was accepted and, if at least [`MOVE_CHECK_TICKS`] ticks ran after it, moved a
//! unit.

use std::collections::BTreeMap;

use bevy::asset::UntypedAssetLoadFailedEvent;
use bevy::prelude::*;
use sim::{
    CellPos, Command, CommandQueue, IssuedCommand, Owner, Position, SimEvent, SimId, SimTick,
    SubPos,
};

use crate::map::MapStatus;
use crate::units::{HUMAN, UnitStatus, starting_unit_total};

/// The cell of the demo map that the smoke test orders the player's units to (grass, across the
/// open ground from their starting area).
pub const MOVE_TARGET: CellPos = CellPos::new(20, 26);

/// Ticks after the order from which some unit must have moved.
pub const MOVE_CHECK_TICKS: u64 = 40;

pub struct SmokeTestPlugin {
    pub frames: u32,
}

impl Plugin for SmokeTestPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SmokeTest {
            frames: self.frames,
            rendered: 0,
            order: None,
            rejected: false,
        })
        .add_systems(Update, (order_move, watch_events))
        .add_systems(Last, (fail_on_asset_load_error, count_frames).chain());
    }
}

#[derive(Resource, Debug)]
struct SmokeTest {
    frames: u32,
    rendered: u32,
    /// The tick at which the move was ordered, and the units' positions then.
    order: Option<(u64, BTreeMap<SimId, SubPos>)>,
    rejected: bool,
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
            error!("smoke test failed: the ordered move was rejected: {reason}");
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

fn count_frames(
    mut smoke: ResMut<SmokeTest>,
    map: Res<MapStatus>,
    status: Res<UnitStatus>,
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
    info!(
        "smoke test: {} frames, {} simulation ticks, map created: {}, units spawned: {}, \
         units moved: {moved} in {ticks_since_order} ticks after the order",
        smoke.rendered, tick.0, map.map_created, status.spawned
    );
    let failures = smoke_failures(&SmokeOutcome {
        frames: smoke.frames,
        map_created: map.map_created,
        spawned: status.spawned,
        expected_units: starting_unit_total(),
        rejected: smoke.rejected,
        ticks_since_order,
        moved,
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
    if outcome.rejected {
        failures.push("the ordered move was rejected".to_string());
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
    fn the_demo_match_expects_all_of_its_starting_units() {
        assert_eq!(starting_unit_total(), 12 + 6 + 5 + 3);
    }
}
