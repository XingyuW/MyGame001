use bevy_ecs::resource::Resource;
use rand_chacha::ChaCha8Rng;
use rand_chacha::rand_core::{Rng, SeedableRng};

use crate::hash::Fnv1a64;

/// The simulation's only source of randomness.
///
/// ChaCha8 is documented by `rand_chacha` as deterministic and portable across platforms, and
/// its full state (key, stream, and word position) is observable, so it can be included in
/// [`state_hash`](crate::state_hash).
#[derive(Resource, Clone, Debug)]
pub struct SimRng(ChaCha8Rng);

impl SimRng {
    pub fn new(seed: u64) -> Self {
        Self(ChaCha8Rng::seed_from_u64(seed))
    }

    pub fn next_u32(&mut self) -> u32 {
        self.0.next_u32()
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0.next_u64()
    }

    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        hasher.write(&self.0.get_seed());
        hasher.write_u64(self.0.get_stream());
        hasher.write_u128(self.0.get_word_pos());
    }
}
