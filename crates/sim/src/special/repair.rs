//! Specialist repair mechanics, moved unchanged from special.rs.
use super::*;

/// Repairs the vehicles docked at a building that repairs units (RA2, docs/RA2_PARITY_PLAN.md
/// section 4.6), in id order: every `unit_repair_interval` ticks a docked vehicle regains
/// `repair_step` hit points (at most what it lacks) for the step cost of its own price and hit
/// points ([`repair_step_cost`](crate::repair_step_cost)). A step its owner cannot pay for is
/// skipped (invariant I7), as is every step while a spy has switched the building off. The
/// vehicle leaves the dock when it is whole (it then moves a cell away, to make room), when it
/// is no longer beside the building or has an order to move, and when the vehicle or the
/// building is lost or changes owner.
pub(super) fn repair_docked(
    world: &mut World,
    defs: &Definitions,
    index: &BTreeMap<SimId, Actor>,
    tick: u64,
) {
    let Some(rules) = defs.rules().map(|rules| rules.economy) else {
        return;
    };
    let docked = world.resource::<SpecialState>().docked.clone();
    let step_now = tick.is_multiple_of(u64::from(rules.unit_repair_interval.max(1).unsigned_abs()));
    for (id, building) in docked {
        let stays = (|| {
            let (a, b) = (index.get(&id)?, index.get(&building)?);
            let ProductionItem::Unit(kind) = a.item else {
                return None;
            };
            let hp = world.get::<Health>(a.entity)?.hit_points;
            let host_hp = world.get::<Health>(b.entity)?.hit_points;
            let owner = world.get::<Owner>(a.entity)?.0;
            let host_owner = world.get::<Owner>(b.entity)?.0;
            let max = defs.unit_combat(kind)?.hit_points;
            let moving = world
                .get::<Orders>(a.entity)
                .is_none_or(|o| o.current.is_some());
            if owner != host_owner
                || !repairs_units(defs, b)
                || distance(a, b) > 1
                || moving
                || hp <= 0
                || host_hp <= 0
            {
                return None;
            }
            if world.resource::<SpecialState>().erasures.contains_key(&id) {
                return Some(());
            }
            let off = world
                .resource::<SpecialState>()
                .disabled
                .contains_key(&building);
            if !off {
                vehicles::free(world, defs, id, true);
            }
            if hp >= max && !world.resource::<SpecialState>().parasites.contains_key(&id) {
                // Whole: it makes room for the next vehicle.
                let dest = {
                    let g = world.resource::<Grid>();
                    let view = g.navigation_for(g.domain_of(id));
                    let on_foot = g.is_on_foot(id);
                    crate::pathfinding::nearest_cell(&view, a.cell, |c| {
                        view.has_room(c, on_foot)
                            && b.foot.is_some_and(|f| f.distance_to_cell(c) >= 2)
                    })
                };
                if let Some(c) = dest
                    && let Some(mut o) = world.get_mut::<Orders>(a.entity)
                {
                    o.current = Some(MoveOrder::new(c, tick));
                }
                return None;
            }
            if step_now && !off {
                let price = defs.production(a.item).map_or(0, |terms| terms.cost);
                let mut players = world.resource_mut::<Players>();
                let state = players.get_mut(owner)?;
                let cost = crate::repair_step_cost(&rules, price, max, state.super_me);
                if state.credits >= cost {
                    state.credits -= cost;
                    let heal = rules.repair_step.max(1).min(max.saturating_sub(hp));
                    if let Some(mut h) = world.get_mut::<Health>(a.entity) {
                        h.hit_points = h.hit_points.saturating_add(heal).min(max);
                    }
                }
            }
            Some(())
        })();
        if stays.is_none() {
            world.resource_mut::<SpecialState>().docked.remove(&id);
        }
    }
}
