use bevy_ecs::world::World;

use crate::ai::AiPlayers;
use crate::building::{Building, Producer, hash_building};
use crate::combat::{Armour, Health, Projectiles, Sight, Weapon, hash_combat};
use crate::definitions::Definitions;
use crate::economy::Players;
use crate::harvest::Harvester;
use crate::id::{SimId, SimIdAllocator};
use crate::map::Grid;
use crate::match_setup::MatchState;
use crate::rng::SimRng;
use crate::schedule::SimTick;
use crate::unit::{Facing, Mobile, Orders, Owner, Position, Unit, hash_unit};

/// 64-bit FNV-1a, implemented here so that the hash never changes with a dependency update.
///
/// Multi-byte integers are written in little-endian order.
#[derive(Clone, Copy, Debug)]
pub struct Fnv1a64(u64);

impl Fnv1a64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    pub fn new() -> Self {
        Self(Self::OFFSET_BASIS)
    }

    pub fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    pub fn write_u32(&mut self, value: u32) {
        self.write(&value.to_le_bytes());
    }

    pub fn write_u64(&mut self, value: u64) {
        self.write(&value.to_le_bytes());
    }

    pub fn write_u128(&mut self, value: u128) {
        self.write(&value.to_le_bytes());
    }

    pub fn finish(&self) -> u64 {
        self.0
    }
}

impl Default for Fnv1a64 {
    fn default() -> Self {
        Self::new()
    }
}

/// Hash of the complete simulation state (invariant I3).
///
/// Covers, in this order: the tick counter, the [`SimRng`] state, the next [`SimId`] to be
/// allocated, the number of entities with a [`SimId`], every such entity in ascending [`SimId`]
/// order (its id, then a flag byte and, for a unit, its components; then a flag byte and, for a
/// harvester, its cycle; then a flag byte and, for a building, its owner and components; then a
/// flag byte and its health, armour, and sight; then a flag byte and its weapon), the [`Grid`],
/// the [`Definitions`], the [`Players`] (with their visibility grids), the projectiles in flight,
/// the [`MatchState`], and the computer players' [`AiPlayers`] (docs/DECISIONS.md D4, D11,
/// extended in M2, M3, M4, and M6). Components added by later milestones are appended per entity.
///
/// # Panics
///
/// If the world was not set up by [`SimPlugin`](crate::SimPlugin).
pub fn state_hash(world: &mut World) -> u64 {
    let mut hasher = Fnv1a64::new();

    hasher.write_u64(world.resource::<SimTick>().0);
    world.resource::<SimRng>().hash_into(&mut hasher);
    hasher.write_u32(world.resource::<SimIdAllocator>().peek_next());

    let mut query = world.query::<(
        &SimId,
        Option<(&Owner, &Unit, &Mobile, &Position, &Facing, &Orders)>,
        Option<&Harvester>,
        Option<(&Owner, &Building, Option<&Producer>)>,
        Option<(&Health, &Armour, &Sight)>,
        Option<&Weapon>,
    )>();
    let mut entities: Vec<_> = query.iter(world).collect();
    entities.sort_unstable_by_key(|(id, ..)| **id);
    hasher.write_u64(entities.len() as u64);
    for (id, unit, harvester, building, combat, weapon) in entities {
        hasher.write_u32(id.0);
        match unit {
            None => hasher.write(&[0]),
            Some(components) => {
                hasher.write(&[1]);
                hash_unit(&mut hasher, components);
            }
        }
        match harvester {
            None => hasher.write(&[0]),
            Some(harvester) => {
                hasher.write(&[1]);
                harvester.hash_into(&mut hasher);
            }
        }
        match building {
            None => hasher.write(&[0]),
            Some((owner, building, producer)) => {
                hasher.write(&[1, owner.0.0]);
                hash_building(&mut hasher, building, producer);
            }
        }
        match combat {
            None => hasher.write(&[0]),
            Some(components) => {
                hasher.write(&[1]);
                hash_combat(&mut hasher, components);
            }
        }
        match weapon {
            None => hasher.write(&[0]),
            Some(weapon) => {
                hasher.write(&[1]);
                weapon.hash_into(&mut hasher);
            }
        }
    }
    world.resource::<Grid>().hash_into(&mut hasher);
    world.resource::<Definitions>().hash_into(&mut hasher);
    world.resource::<Players>().hash_into(&mut hasher);
    world.resource::<Projectiles>().hash_into(&mut hasher);
    world.resource::<MatchState>().hash_into(&mut hasher);
    world.resource::<AiPlayers>().hash_into(&mut hasher);

    world
        .resource::<crate::SpecialState>()
        .hash_into(&mut hasher);
    world.resource::<crate::Props>().hash_into(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::Fnv1a64;

    fn fnv(bytes: &[u8]) -> u64 {
        let mut hasher = Fnv1a64::new();
        hasher.write(bytes);
        hasher.finish()
    }

    #[test]
    fn fnv1a64_matches_reference_vectors() {
        assert_eq!(fnv(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn integer_writes_are_little_endian() {
        let mut a = Fnv1a64::new();
        a.write_u32(0x0403_0201);
        assert_eq!(a.finish(), fnv(&[1, 2, 3, 4]));
    }
}
