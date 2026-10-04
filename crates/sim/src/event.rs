//! Simulation events (PLAN.md section 5: `SimEvent`).
//!
//! `SimEvent` is a Bevy `Message` (docs/DECISIONS.md A1: buffered events are messages in Bevy
//! 0.19). The simulation owns its message buffer: [`SimPlugin`](crate::SimPlugin) inserts it
//! without registering it for Bevy's per-frame update, and the first system of every tick
//! advances it. So after a tick, `Messages::<SimEvent>::iter_current_update_messages` yields
//! exactly that tick's events, and a `MessageReader` sees the events of the last two ticks.

use bevy_ecs::message::{Message, Messages};
use bevy_ecs::system::ResMut;

use crate::command::{Command, PlayerId, RejectReason};
use crate::id::SimId;

/// Something that happened during a tick.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub enum SimEvent {
    /// The command was invalid and changed no state.
    CommandRejected {
        player: PlayerId,
        command: Command,
        reason: RejectReason,
    },
    /// The unit reached the destination of its move order.
    MoveCompleted { unit: SimId },
    /// The unit's move order ended without reaching its destination; the unit stops at the next
    /// cell centre.
    MoveFailed { unit: SimId, reason: MoveFailure },
}

/// Why a move order ended early.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MoveFailure {
    /// A group order had fewer free cells reachable from the target than units; this unit got
    /// none.
    NoDestination,
    /// No path over passable terrain leads to the unit's destination.
    Unreachable,
    /// Other units kept the way blocked: repeated repaths around them found no way.
    Blocked,
}

/// Starts the tick's event buffer (first system of every tick).
pub(crate) fn advance_events(mut events: ResMut<Messages<SimEvent>>) {
    events.update();
}
