//! Simulation events (PLAN.md section 5: `SimEvent`).
//!
//! `SimEvent` is a Bevy `Message` (docs/DECISIONS.md A1: buffered events are messages in Bevy
//! 0.19). The simulation owns its message buffer: [`SimPlugin`](crate::SimPlugin) inserts it
//! without registering it for Bevy's per-frame update, and the first system of every tick
//! advances it. So after a tick, `Messages::<SimEvent>::iter_current_update_messages` yields
//! exactly that tick's events, and a `MessageReader` sees the events of the last two ticks.

use bevy_ecs::message::{Message, Messages};
use bevy_ecs::system::ResMut;

use crate::combat::DamageState;
use crate::combat_defs::{WarheadKind, WeaponKind};
use crate::command::{Command, PlayerId, RejectReason};
use crate::definitions::UnitKind;
use crate::economy_defs::{BuildingKind, ProductionItem};
use crate::id::SimId;
use crate::map::{CellPos, SubPos};

/// Something that happened during a tick.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub enum SimEvent {
    /// Public location notice; radar display still follows powered-radar policy.
    BuildingRevealed {
        player: PlayerId,
        building: SimId,
        kind: BuildingKind,
        origin: CellPos,
    },
    SuperweaponBuilt {
        player: PlayerId,
        role: crate::Role,
    },
    SuperweaponReady {
        player: PlayerId,
        role: crate::Role,
    },
    SuperweaponLaunched {
        player: PlayerId,
        role: crate::Role,
    },
    BridgeChanged {
        bridge: SimId,
        cells: Vec<CellPos>,
        intact: bool,
    },
    StrategicActivated {
        player: PlayerId,
        building: SimId,
        target: crate::CellPos,
        role: crate::Role,
        radius: u32,
    },
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
    MoveCompleted {
        unit: SimId,
        retargeted: bool,
    },
    /// The unit's move order ended without reaching its destination; the unit stops at the next
    /// cell centre.
    MoveFailed {
        unit: SimId,
        reason: MoveFailure,
    },
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
    /// A ready placed Super Me unit was put on the map (`Command::PlaceUnit`, docs/DECISIONS.md
    /// R218).
    UnitPlaced {
        player: PlayerId,
        unit: SimId,
        kind: crate::UnitKind,
    },
    /// A ready building was placed.
    BuildingPlaced {
        player: PlayerId,
        building: SimId,
        kind: BuildingKind,
    },
    /// The Construction Yard `building` of type `kind` was undeployed (`Command::Undeploy`, RA1):
    /// its cells were freed and it is now the unit `unit` (the same id) of type `into`, standing
    /// in the cell of the footprint's origin.
    BuildingUndeployed {
        player: PlayerId,
        building: SimId,
        kind: BuildingKind,
        unit: SimId,
        into: crate::UnitKind,
    },
    /// The building was sold: it was removed, its cells were freed, and `refund` credits were
    /// paid.
    BuildingSold {
        player: PlayerId,
        building: SimId,
        refund: i64,
    },
    /// A harvester unloaded `ore` units (bails since RA5, `gems` of them bails of gems) at a
    /// refinery, which paid `credits`.
    OreDelivered {
        player: PlayerId,
        harvester: SimId,
        refinery: SimId,
        ore: u32,
        gems: u32,
        credits: i64,
    },
    /// The harvester stopped its cycle and waits for a `Harvest` command.
    HarvesterIdle {
        unit: SimId,
        reason: HarvestIdle,
    },
    /// `attacker` fired `weapon` at `target`. An instant weapon's damage is dealt in this tick, a
    /// projectile's when it arrives (M4).
    WeaponFired {
        attacker: SimId,
        target: SimId,
        weapon: WeaponKind,
    },
    /// A projectile arrived after its target had been removed (destroyed or sold), so it dealt no
    /// damage.
    ProjectileExpired {
        attacker: SimId,
        target: SimId,
        weapon: WeaponKind,
    },
    /// A warhead went off at `at`, in the air or on the ground, where no shot's event says it
    /// would (RA3): the impact of an inaccurate shot, of a projectile whose target was gone, of
    /// a bomb, a suicide unit or a superweapon. The impact of every other shot is where the
    /// target of its [`SimEvent::WeaponFired`] is, and is not reported.
    Impact {
        warhead: WarheadKind,
        at: SubPos,
        air: bool,
    },
    /// The damage state of `player`'s unit or building `id` changed (RA3, with a rules file):
    /// it fell to or below, or rose above, the yellow or the red share of its hit points.
    DamageStateChanged {
        player: PlayerId,
        id: SimId,
        state: DamageState,
    },
    /// `player`'s infantry unit `unit` was crushed by the vehicle `crusher` of the player `by`,
    /// which drove into its cell (RA3, docs/DECISIONS.md R6); `at` is where it stood in that
    /// cell. The unit is gone at once: [`SimEvent::UnitDestroyed`] follows in the same tick.
    /// `by` is its killer (for the experience of RA4).
    UnitCrushed {
        player: PlayerId,
        unit: SimId,
        crusher: SimId,
        by: PlayerId,
        at: SubPos,
    },
    /// `player`'s unit `unit` rose to `rank` (RA4, `crate::veterancy`): 1 veteran, 2 elite.
    RankGained {
        player: PlayerId,
        unit: SimId,
        rank: u8,
    },
    /// A crate appeared in `cell` (RA5, `crate::crates`).
    CrateAppeared {
        cell: CellPos,
    },
    /// The crate in `cell` is gone without having been taken (RA5): its time was up, or a
    /// building was placed on it.
    CrateGone {
        cell: CellPos,
    },
    /// `player`'s unit `unit` took the crate in `cell` (RA5): what it held in the end
    /// (`content`: money where another content could not be given), the credits it paid, and
    /// the unit that came out of it.
    CrateTaken {
        player: PlayerId,
        unit: SimId,
        cell: CellPos,
        content: crate::crates::CrateContent,
        credits: i64,
        spawned: Option<SimId>,
    },
    /// The crew or the survivors of `player`'s destroyed vehicle, or destroyed or sold building,
    /// `from` left it (RA4, `crate::crew`): the new units `units`, standing in and around
    /// `at`. `from` no longer exists.
    CrewEscaped {
        player: PlayerId,
        from: SimId,
        at: CellPos,
        units: Vec<SimId>,
    },
    /// `player`'s unit `parent` launched the unit `unit`: a missile, or an aircraft it carries
    /// (RA4, `crate::flight`). The unit is in the air above `parent` and cannot be commanded.
    UnitLaunched {
        player: PlayerId,
        parent: SimId,
        unit: SimId,
    },
    /// The aircraft `unit` that `player`'s unit `parent` carries came back and was taken
    /// aboard (RA4): it is no longer on the map.
    UnitRecovered {
        player: PlayerId,
        parent: SimId,
        unit: SimId,
    },
    /// The missile `unit` that `player`'s unit `parent` launched reached the end of its flight
    /// and went off at `at` (RA4): it is no longer on the map. A [`SimEvent::Impact`] of the
    /// same tick reports the warhead.
    MissileDetonated {
        player: PlayerId,
        parent: SimId,
        unit: SimId,
        at: SubPos,
    },
    /// The unit's hit points fell to 0 or below, or it was crushed: it was removed and its cells
    /// were freed.
    UnitDestroyed {
        player: PlayerId,
        unit: SimId,
    },
    /// The building's hit points fell to 0 or below: it was removed and its cells were freed.
    /// `lost_units` are the free units that were still waiting to leave it; they are lost with it.
    BuildingDestroyed {
        player: PlayerId,
        building: SimId,
        kind: BuildingKind,
        lost_units: Vec<UnitKind>,
    },
    /// The unit's `Attack` order on `target` ended without a new command; the unit stops at its
    /// next cell centre.
    AttackEnded {
        unit: SimId,
        target: SimId,
        reason: AttackEnd,
    },
    /// The scenery in `cell` (`crate::props`) lost its last hit point: it was removed and the
    /// cell is open ground.
    PropDestroyed {
        prop: SimId,
        cell: CellPos,
        kind: crate::props::PropKind,
    },
    /// The player of the match is defeated (M6, docs/DECISIONS.md D41): it owns no building any
    /// more, and with a rules file (RA1) no base unit either in a short game, or no unit at all
    /// in a long one.
    PlayerDefeated {
        player: PlayerId,
    },
    /// At most one player of the match is left: `winner`, or `None` for a draw. The outcome is
    /// final; it is written once.
    MatchEnded {
        winner: Option<PlayerId>,
    },
}

/// Why an `Attack` order ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AttackEnd {
    /// The target no longer exists (destroyed, or sold by its owner).
    TargetGone,
    /// The target is no longer visible to the unit's owner.
    TargetHidden,
    /// The unit moved toward the target `MAX_CHASES` times without firing in between, or no free
    /// cell from which the target is in range is reachable.
    OutOfReach,
    /// The unit's warhead deals no damage to the target's armour (equation 2 with a percentage
    /// of 0): the unit fired once, as ordered, and the order ended with that shot
    /// (docs/DECISIONS.md D32).
    NoDamage,
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
