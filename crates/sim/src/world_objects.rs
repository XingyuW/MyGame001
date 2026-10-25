//! Map-owned objects installed identically for interactive and headless matches (RA5).
//! Neutral is a reserved owner, outside the competing Players and MatchState collections.
use crate::{Building, Definitions, Footprint, Grid, Owner, PlayerId, ProductionItem, SimId};
use bevy_ecs::world::World;

/// Passive world owner: no economy, team, defeat, or victory.
pub const NEUTRAL: PlayerId = PlayerId(u8::MAX);
/// Stable map-object IDs do not consume the unit/building allocator.
pub const WORLD_ID_BASE: u32 = 0xA000_0000;

/// Scenario setup, before ticking: installs the retained objects in client and headless worlds.
pub fn install(world: &mut World, bridge_destruction: bool) {
    let definitions = world.resource::<Definitions>().clone();
    if world.resource::<Grid>().world_features().next().is_some()
        && let Some(armour) = definitions
            .armour_class("wood")
            .or_else(|| definitions.armour_class("structure"))
    {
        let mut props = crate::Props::from_grid(world.resource::<Grid>(), armour);
        props.configure(&definitions);
        props.bridge_destructible = bridge_destruction;
        world.insert_resource(props);
    }
    let features: Vec<_> = world.resource::<Grid>().world_features().collect();
    for (cell, feature) in features {
        let name = match feature {
            11 => "oil_derrick",
            12 => "hospital",
            13 => "airport",
            14 => "tech_outpost",
            15 => "bridge_hut",
            _ => continue,
        };
        place(world, name, cell);
    }
    crate::bridges::install(world);
}

pub(crate) fn place(world: &mut World, name: &str, cell: crate::CellPos) -> Option<SimId> {
    let definitions = world.resource::<Definitions>().clone();
    let kind = definitions.building_kind(name)?;
    let terms = definitions.building_combat(kind)?;
    let grid = world.resource::<Grid>();
    if !grid.contains(cell)
        || grid.building(cell).is_some()
        || grid.holders(cell).into_iter().flatten().next().is_some()
    {
        return None;
    }
    let index = u32::try_from(cell.y)
        .ok()?
        .checked_mul(grid.width())?
        .checked_add(u32::try_from(cell.x).ok()?)?;
    let id = SimId(WORLD_ID_BASE.checked_add(index)?);
    let footprint = Footprint {
        origin: cell,
        width: 1,
        height: 1,
    };
    let (combat, weapon) = crate::combat::combat_components(terms);
    let mut entity = world.spawn((
        id,
        Owner(NEUTRAL),
        Building {
            kind,
            footprint,
            pending_units: Vec::new(),
        },
        combat,
    ));
    if let Some(weapon) = weapon {
        entity.insert(weapon);
    }
    world.resource_mut::<Grid>().cover(cell, id);
    Some(id)
}

pub(crate) fn immune(definitions: &Definitions, item: Option<ProductionItem>) -> bool {
    item.and_then(|item| definitions.metadata(item))
        .is_some_and(|entry| entry.immune)
}

/// Tick deadlines for captured cash-producing buildings. Empty in older fixtures.
#[derive(bevy_ecs::resource::Resource, Default, Clone, Debug)]
pub struct WorldObjects {
    pub(crate) cash_due: std::collections::BTreeMap<SimId, u64>,
}
impl WorldObjects {
    pub(crate) fn hash_into(&self, h: &mut crate::Fnv1a64) {
        if !self.cash_due.is_empty() {
            h.write(b"tech_cash");
            h.write_u64(self.cash_due.len() as u64);
            for (id, due) in &self.cash_due {
                h.write_u32(id.0);
                h.write_u64(*due);
            }
        }
    }
}

pub(crate) fn captured(
    world: &mut World,
    definitions: &Definitions,
    id: SimId,
    item: ProductionItem,
    previous: PlayerId,
    owner: PlayerId,
) {
    let Some(entry) = definitions.metadata(item) else {
        return;
    };
    if previous == NEUTRAL
        && let Some(player) = world.resource_mut::<crate::Players>().get_mut(owner)
    {
        player.credits = player.credits.saturating_add(i64::from(entry.cash_startup));
    }
    if entry.cash_amount > 0 && entry.cash_delay > 0 {
        let due = world
            .resource::<crate::SimTick>()
            .0
            .saturating_add(u64::from(entry.cash_delay));
        world
            .resource_mut::<WorldObjects>()
            .cash_due
            .insert(id, due);
    }
}

pub(crate) fn advance(world: &mut World) {
    if world.resource::<WorldObjects>().cash_due.is_empty() {
        return;
    }
    let definitions = world.resource::<Definitions>().clone();
    let tick = world.resource::<crate::SimTick>().0;
    let index: std::collections::BTreeMap<_, _> = world
        .query::<(&SimId, &Owner, &Building, &crate::Health)>()
        .iter(world)
        .filter(|(_, owner, _, hp)| owner.0 != NEUTRAL && hp.hit_points > 0)
        .map(|(id, owner, building, _)| (*id, (owner.0, building.kind)))
        .collect();
    let pending = world.resource::<WorldObjects>().cash_due.clone();
    for (id, due) in pending {
        let Some(&(owner, kind)) = index.get(&id) else {
            world.resource_mut::<WorldObjects>().cash_due.remove(&id);
            continue;
        };
        if tick < due {
            continue;
        }
        let Some(entry) = definitions.metadata(ProductionItem::Building(kind)) else {
            continue;
        };
        if let Some(player) = world.resource_mut::<crate::Players>().get_mut(owner) {
            player.credits = player.credits.saturating_add(i64::from(entry.cash_amount));
        }
        world
            .resource_mut::<WorldObjects>()
            .cash_due
            .insert(id, tick.saturating_add(u64::from(entry.cash_delay.max(1))));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ra5_cash_deadline_and_host_identity_are_hashed() {
        let hash = |state: &WorldObjects| {
            let mut h = crate::Fnv1a64::new();
            state.hash_into(&mut h);
            h.finish()
        };
        let mut state = WorldObjects::default();
        let empty = hash(&state);
        state.cash_due.insert(SimId(7), 133);
        let original = hash(&state);
        assert_ne!(empty, original);
        state.cash_due.insert(SimId(7), 134);
        assert_ne!(hash(&state), original);
        state.cash_due.clear();
        state.cash_due.insert(SimId(8), 133);
        assert_ne!(hash(&state), original);
        state.cash_due.clear();
        state.cash_due.insert(SimId(7), 133);
        assert_eq!(hash(&state), original);
    }
}
