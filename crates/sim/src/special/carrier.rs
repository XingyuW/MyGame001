//! Specialist carrier mechanics, moved unchanged from special.rs.
use super::*;

/// Carrier sorties reuse the ordinary aircraft movement, targeting and damage path.
pub(super) fn launch_fighters(
    world: &mut World,
    carrier: &Actor,
    target: &Actor,
    defs: &Definitions,
    capacity: usize,
) {
    let Some(kind) = defs.arsenal().0.iter().find_map(|(item, meta)| {
        if meta.role == Role::Strike
            && let ProductionItem::Unit(k) = item
        {
            return Some(*k);
        }
        None
    }) else {
        return;
    };
    let Some(unit) = defs.unit(kind) else {
        return;
    };
    let active = world
        .resource::<SpecialState>()
        .carrier_flights
        .values()
        .filter(|id| **id == carrier.id)
        .count();
    for _ in active..capacity {
        let cell = {
            let grid = world.resource::<Grid>();
            let view = grid.navigation_for(Domain::Air);
            crate::building::exit_cell(
                &view,
                Footprint {
                    origin: carrier.cell,
                    width: 1,
                    height: 1,
                },
                None,
                false,
            )
        };
        let Some(cell) = cell else {
            break;
        };
        let Ok(id) = crate::spawn_unit(world, carrier.owner, &unit.id, cell) else {
            break;
        };
        let entity = world
            .query::<(Entity, &SimId)>()
            .iter(world)
            .find(|(_, i)| **i == id)
            .map(|(e, _)| e)
            .unwrap();
        if let Some(mut weapon) = world.get_mut::<Weapon>(entity) {
            weapon.order = Some(crate::combat::AttackOrder::Attack {
                target: target.id,
                chases: 0,
            });
        }
        world
            .resource_mut::<SpecialState>()
            .carrier_flights
            .insert(id, carrier.id);
    }
}
