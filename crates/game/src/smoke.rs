//! `--smoke-test <frames>`: run the real application for a fixed number of frames, then exit.
//!
//! Exits with code 0 only if no asset failed to load, the map was validated and spawned, the
//! starting units were spawned, and the move that the smoke test orders through `CommandQueue`
//! was accepted and, if at least [`MOVE_CHECK_TICKS`] ticks ran after it, moved a unit.

use std::collections::BTreeMap;

use bevy::asset::UntypedAssetLoadFailedEvent;
use bevy::prelude::*;
use sim::{
    CellPos, Command, CommandQueue, IssuedCommand, Owner, Position, SimEvent, SimId, SimTick,
    SubPos,
};

use crate::map::MapStatus;
use crate::units::{HUMAN, UnitStatus};

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
    let mut failures = Vec::new();
    if !map.map_created {
        failures.push(format!(
            "the map was not created within {} frames",
            smoke.frames
        ));
    }
    if status.spawned == 0 {
        failures.push("no unit was spawned".to_string());
    }
    if smoke.rejected {
        failures.push("the ordered move was rejected".to_string());
    }
    if ticks_since_order >= MOVE_CHECK_TICKS && moved == 0 {
        failures.push(format!(
            "no unit moved within {ticks_since_order} ticks of the order"
        ));
    }
    if failures.is_empty() {
        exit.write(AppExit::Success);
    } else {
        for failure in failures {
            error!("smoke test failed: {failure}");
        }
        exit.write(AppExit::error());
    }
}
