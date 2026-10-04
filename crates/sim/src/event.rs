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
use crate::economy_defs::{BuildingKind, ProductionItem};
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
    /// The unit reached the destination of its move order. `retargeted` is `false` when that is
    /// the destination the order assigned, and `true` when units that stand still closed every
    /// way to it and the unit stopped instead at the reachable cell nearest to it
    /// (docs/DECISIONS.md D19 item 6, D21).
    MoveCompleted { unit: SimId, retargeted: bool },
    /// The unit's move order ended without reaching its destination; the unit stops at the next
    /// cell centre.
    MoveFailed { unit: SimId, reason: MoveFailure },
    /// The item at the head of one of the player's production queues has made its last
    /// progress, so its cost has been charged in full: a building is now ready to be placed, and
    /// a unit leaves a producing building as soon as one of its ring cells is free
    /// ([`SimEvent::UnitSpawned`]).
    ProductionComplete {
        player: PlayerId,
        item: ProductionItem,
    },
    /// A queued item was cancelled and the credits charged for it so far were refunded.
    ProductionCancelled {
        player: PlayerId,
        item: ProductionItem,
        refund: i64,
    },
    /// A produced unit, or a free unit of a building placed during the match, left `building`.
    UnitSpawned {
        player: PlayerId,
        unit: SimId,
        building: SimId,
    },
    /// A ready building was placed.
    BuildingPlaced {
        player: PlayerId,
        building: SimId,
        kind: BuildingKind,
    },
    /// The building was sold: it was removed, its cells were freed, and `refund` credits were
    /// paid.
    BuildingSold {
        player: PlayerId,
        building: SimId,
        refund: i64,
    },
    /// A harvester unloaded `ore` units at a refinery, which paid `credits`.
    OreDelivered {
        player: PlayerId,
        harvester: SimId,
        refinery: SimId,
        ore: u32,
        credits: i64,
    },
    /// The harvester stopped its cycle and waits for a `Harvest` command.
    HarvesterIdle { unit: SimId, reason: HarvestIdle },
}

/// Why a harvester stopped its cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HarvestIdle {
    /// No cell with ore is reachable over passable terrain.
    NoOre,
    /// It carries ore, and no refinery of its owner is reachable (or its owner is not a player
    /// of the match).
    NoRefinery,
    /// It set out `MAX_HARVEST_ATTEMPTS` times without harvesting or unloading in between
    /// (other units kept the way or the cells taken).
    Blocked,
}

/// Why a move order ended early.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MoveFailure {
    /// A group order had fewer free cells reachable from the target than units; this unit got
    /// none.
    NoDestination,
    /// No path over passable terrain leads to the unit's destination.
    Unreachable,
    /// Other units kept the way blocked: the unit waited `MAX_STALLED_TICKS` ticks at a cell
    /// centre for cells held by units that were not moving on, without a step toward its
    /// destination in between (its own steps, waits for the pathfinder, and waits behind units
    /// that were moving on do not count), or took more than `MAX_PATHS_WITHOUT_PROGRESS` new
    /// paths around them without getting closer than before.
    Blocked,
}

/// Starts the tick's event buffer (first system of every tick).
pub(crate) fn advance_events(mut events: ResMut<Messages<SimEvent>>) {
    events.update();
}
