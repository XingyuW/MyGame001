//! Crews and survivors (RA4, docs/RA2_PARITY_PLAN.md section 4.7; docs/DECISIONS.md R48).
//!
//! Only with a rules file, and only for types with the arsenal's `crewed` trait. Whatever is
//! destroyed (`remove_dead`) or sold (`SellBuilding`) in a tick is recorded as a [`Wreck`], and
//! [`escape`] lets its crew out at the end of the `Cleanup` stage, in the order of the records
//! (the sales of the tick in command order, then the destroyed in id order):
//!
//! * **A vehicle** that was not in a step when it was destroyed releases, with the rules'
//!   `crew_escape_percent` probability, one unit of the crew type of its owner's side, in its
//!   cell or the nearest cell around it with room. An aircraft releases nobody.
//! * **A building**, destroyed or sold, releases `max(1, ceil(refund / survivor_divisor))`
//!   units of that crew type, `refund` being what a sale of it pays and the divisor that of
//!   its owner's side, into the cells of its footprint, as many as have room (three soldiers
//!   on foot to a cell). A construction yard (a factory of buildings) also releases an
//!   engineer (the first unit type with the role `engineer`) with a probability of
//!   [`ENGINEER_ESCAPE_PERCENT`].
//!
//! The owner's side is that of its country; an owner without a country has no crew. The
//! probabilities are drawn from [`SimRng`], one draw per crewed vehicle that stood still and one
//! per construction yard, so a world without crewed types draws nothing. The new units are
//! reported with [`SimEvent::CrewEscaped`].

use bevy_ecs::message::Messages;
use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;

use crate::arsenal::{Domain, FactoryGroup, Role};
use crate::building::Footprint;
use crate::command::PlayerId;
use crate::definitions::{Definitions, UnitKind};
use crate::economy::Players;
use crate::economy_defs::{BuildingKind, ProductionItem};
use crate::event::SimEvent;
use crate::hash::Fnv1a64;
use crate::id::SimId;
use crate::map::{CellPos, Grid};
use crate::rng::SimRng;

/// The probability in percent that an engineer leaves a destroyed or sold construction yard
/// (hard coded in the original engine and its ancestor; it is in no ini).
pub const ENGINEER_ESCAPE_PERCENT: u32 = 25;

/// What a destroyed or sold unit or building was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Wrecked {
    /// A unit, in `cell`; `moving`: it was in a step.
    Unit {
        kind: UnitKind,
        cell: CellPos,
        moving: bool,
    },
    Building {
        kind: BuildingKind,
        footprint: Footprint,
    },
}

/// A unit or building that was destroyed or sold in this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Wreck {
    pub(crate) owner: PlayerId,
    pub(crate) id: SimId,
    pub(crate) what: Wrecked,
}

/// The wrecks of the tick, in the order they were made; empty between ticks.
#[derive(Resource, Clone, Debug, Default)]
pub(crate) struct Wrecks(pub(crate) Vec<Wreck>);

impl Wrecks {
    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        if self.0.is_empty() {
            return;
        }
        hasher.write(b"wrecks");
        hasher.write_u64(self.0.len() as u64);
        for wreck in &self.0 {
            hasher.write(&[wreck.owner.0]);
            hasher.write_u32(wreck.id.0);
            match wreck.what {
                Wrecked::Unit { kind, cell, moving } => {
                    hasher.write(&[0, u8::from(moving)]);
                    hasher.write(&kind.0.to_le_bytes());
                    hasher.write(&cell.x.to_le_bytes());
                    hasher.write(&cell.y.to_le_bytes());
                }
                Wrecked::Building { kind, footprint } => {
                    hasher.write(&[1, footprint.width, footprint.height]);
                    hasher.write(&kind.0.to_le_bytes());
                    hasher.write(&footprint.origin.x.to_le_bytes());
                    hasher.write(&footprint.origin.y.to_le_bytes());
                }
            }
        }
    }
}

/// The survivors of a building whose sale pays `refund`, for an owner whose side has the
/// divisor `divisor`: `max(1, ceil(refund / divisor))` (docs/RA2_PARITY_PLAN.md section 4.7).
pub fn survivors(refund: i64, divisor: i32) -> u32 {
    let divisor = i64::from(divisor.max(1));
    let count = refund.max(0).saturating_add(divisor - 1) / divisor;
    u32::try_from(count.max(1)).unwrap_or(u32::MAX)
}

/// Whether an event of `percent` percent happens: one draw from `rng`.
fn chance(rng: &mut SimRng, percent: u32) -> bool {
    rng.next_u32() % 100 < percent
}

/// Puts a unit of type `kind` of `owner` into the first of `cells` that has room for it.
fn put(world: &mut World, owner: PlayerId, kind: UnitKind, cells: &[CellPos]) -> Option<SimId> {
    let id = world.resource::<Definitions>().unit(kind)?.id.clone();
    cells
        .iter()
        .find_map(|&cell| crate::spawn_unit(world, owner, &id, cell).ok())
}

/// Lets the crews and survivors of the tick's wrecks out (see the module documentation).
pub(crate) fn escape(world: &mut World) {
    let wrecks = std::mem::take(&mut world.resource_mut::<Wrecks>().0);
    if wrecks.is_empty() {
        return;
    }
    let definitions = world.resource::<Definitions>().clone();
    let Some(rules) = definitions.rules() else {
        return;
    };
    let engineer = definitions
        .arsenal()
        .0
        .iter()
        .find_map(|(item, entry)| match item {
            ProductionItem::Unit(kind) if entry.role == Role::Engineer => Some(*kind),
            _ => None,
        });
    for wreck in wrecks {
        let item = match wreck.what {
            Wrecked::Unit { kind, .. } => ProductionItem::Unit(kind),
            Wrecked::Building { kind, .. } => ProductionItem::Building(kind),
        };
        if !definitions.metadata(item).is_some_and(|entry| entry.crewed) {
            continue;
        }
        // The crew is of the side of the owner's country.
        let side = world
            .resource::<Players>()
            .get(wreck.owner)
            .and_then(|state| state.country())
            .and_then(|country| rules.country(country))
            .and_then(|country| rules.side(country.side));
        let Some(side) = side else {
            continue;
        };
        let mut units = Vec::new();
        let at = match wreck.what {
            Wrecked::Unit { kind, cell, moving } => {
                if definitions.domain(kind) == Domain::Air || moving {
                    continue;
                }
                let percent = u32::try_from(rules.crew.crew_escape_percent.max(0)).unwrap_or(0);
                if !chance(&mut world.resource_mut::<SimRng>(), percent) {
                    continue;
                }
                let mut cells = vec![cell];
                cells.extend(
                    Footprint {
                        origin: cell,
                        width: 1,
                        height: 1,
                    }
                    .ring()
                    .filter(|&cell| world.resource::<Grid>().contains(cell)),
                );
                units.extend(put(world, wreck.owner, side.crew, &cells));
                cell
            }
            Wrecked::Building { kind, footprint } => {
                let count = survivors(definitions.refund(kind), side.survivor_divisor);
                let cells: Vec<CellPos> = footprint.cells().collect();
                for _ in 0..count {
                    match put(world, wreck.owner, side.crew, &cells) {
                        Some(unit) => units.push(unit),
                        None => break,
                    }
                }
                if definitions.factory_group(kind) == Some(FactoryGroup::Buildings)
                    && chance(&mut world.resource_mut::<SimRng>(), ENGINEER_ESCAPE_PERCENT)
                    && let Some(engineer) = engineer
                {
                    units.extend(put(world, wreck.owner, engineer, &cells));
                }
                footprint.origin
            }
        };
        if !units.is_empty() {
            world
                .resource_mut::<Messages<SimEvent>>()
                .write(SimEvent::CrewEscaped {
                    player: wreck.owner,
                    from: wreck.id,
                    at,
                    units,
                });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Section 4.7: the count is the refund over the divisor, rounded up, at least 1.
    #[test]
    fn the_survivors_are_the_refund_over_the_divisor_rounded_up() {
        assert_eq!(survivors(1500, 500), 3);
        assert_eq!(survivors(1500, 250), 6);
        assert_eq!(survivors(400, 500), 1);
        assert_eq!(survivors(400, 250), 2);
        assert_eq!(survivors(500, 500), 1);
        assert_eq!(survivors(501, 500), 2);
        assert_eq!(survivors(0, 500), 1, "at least 1");
        assert_eq!(survivors(-5, 500), 1);
        assert_eq!(survivors(1000, 0), 1000, "a divisor of 0 counts as 1");
        assert_eq!(survivors(i64::MAX, 1), u32::MAX);
    }
}
