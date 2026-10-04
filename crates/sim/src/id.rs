use bevy_ecs::component::Component;
use bevy_ecs::resource::Resource;

/// Stable simulation identifier, assigned monotonically by [`SimIdAllocator`].
///
/// All order sensitive iteration sorts by this id, never by `Entity` or query order.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SimId(pub u32);

/// Hands out [`SimId`] values in increasing order, never reusing one.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SimIdAllocator {
    next: u32,
}

impl SimIdAllocator {
    /// Returns the next id, or `None` once the id space is exhausted (`u32::MAX` is never
    /// handed out, so the allocator itself cannot overflow).
    pub fn allocate(&mut self) -> Option<SimId> {
        let id = SimId(self.next);
        self.next = self.next.checked_add(1)?;
        Some(id)
    }

    /// The id that the next call to [`allocate`](Self::allocate) will return.
    pub fn peek_next(&self) -> u32 {
        self.next
    }
}
