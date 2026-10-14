//! Connected bridge decks share health. Collapse and Engineer restoration change navigation
//! only in scheduled simulation stages; huts are neutral and immune to ordinary damage.
use crate::{Building, Cell, CellPos, Definitions, Grid, ProductionItem, Props, SimId};
use bevy_ecs::{resource::Resource, world::World};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct Bridge {
    pub cells: Vec<CellPos>,
    pub collapsed: bool,
    pub huts: BTreeSet<SimId>,
    decks: Vec<Cell>,
}
#[derive(Resource, Clone, Debug, Default)]
pub struct Bridges {
    pub bridges: BTreeMap<SimId, Bridge>,
    pub huts: BTreeMap<SimId, SimId>,
}
impl Bridges {
    pub(crate) fn hash_into(&self, h: &mut crate::Fnv1a64) {
        if self.bridges.is_empty() {
            return;
        }
        h.write(b"bridges");
        h.write_u64(self.bridges.len() as u64);
        for (id, bridge) in &self.bridges {
            h.write_u32(id.0);
            h.write(&[u8::from(bridge.collapsed)]);
            h.write_u64(bridge.cells.len() as u64);
            for (cell, deck) in bridge.cells.iter().zip(&bridge.decks) {
                h.write(&cell.x.to_le_bytes());
                h.write(&cell.y.to_le_bytes());
                h.write(&[
                    u8::from(deck.passable),
                    u8::from(deck.water),
                    u8::from(deck.buildable),
                ]);
                h.write_u32(deck.ore);
            }
            h.write_u64(bridge.huts.len() as u64);
            for hut in &bridge.huts {
                h.write_u32(hut.0);
            }
        }
        h.write_u64(self.huts.len() as u64);
        for (hut, bridge) in &self.huts {
            h.write_u32(hut.0);
            h.write_u32(bridge.0);
        }
    }
}
pub(crate) fn install(world: &mut World) {
    let mut groups: BTreeMap<CellPos, Vec<CellPos>> = BTreeMap::new();
    for (cell, anchor) in world.resource::<Props>().bridge_cells() {
        groups.entry(anchor).or_default().push(cell);
    }
    let defs = world.resource::<Definitions>().clone();
    let mut state = Bridges::default();
    let huts: Vec<_> = world
        .query::<(&SimId, &Building)>()
        .iter(world)
        .filter(|(_, b)| {
            defs.metadata(ProductionItem::Building(b.kind))
                .is_some_and(|m| m.bridge_repair_hut)
        })
        .map(|(id, b)| (*id, b.footprint.origin))
        .collect();
    for (anchor, cells) in groups {
        let Some(id) = world.resource::<Props>().id_at(anchor) else {
            continue;
        };
        let decks = cells
            .iter()
            .filter_map(|&c| world.resource::<Grid>().cell(c).copied())
            .collect();
        state.bridges.insert(
            id,
            Bridge {
                cells,
                decks,
                collapsed: false,
                huts: BTreeSet::new(),
            },
        );
    }
    for (hut, cell) in huts {
        let nearest = state
            .bridges
            .iter()
            .filter_map(|(id, b)| {
                b.cells
                    .iter()
                    .map(|c| c.x.abs_diff(cell.x).max(c.y.abs_diff(cell.y)))
                    .min()
                    .map(|distance| (distance, *id))
            })
            .min();
        if let Some((distance, id)) = nearest
            && distance <= 6
        {
            state.huts.insert(hut, id);
            if let Some(b) = state.bridges.get_mut(&id) {
                b.huts.insert(hut);
            }
        }
    }
    // Older maps have only deck looks. Add a hut off the travelling lane at each end, keeping
    // every original cell and test coordinate intact. Generated maps carry explicit anchors.
    for (id, b) in &mut state.bridges {
        let Some(min_x) = b.cells.iter().map(|c| c.x).min() else {
            continue;
        };
        let max_x = b.cells.iter().map(|c| c.x).max().unwrap_or(min_x);
        let min_y = b.cells.iter().map(|c| c.y).min().unwrap_or(0);
        let max_y = b.cells.iter().map(|c| c.y).max().unwrap_or(min_y);
        let horizontal = max_x - min_x >= max_y - min_y;
        let ends = [false, true].map(|last| {
            let edge = if horizontal {
                if last { max_x } else { min_x }
            } else if last {
                max_y
            } else {
                min_y
            };
            let across: Vec<_> = b
                .cells
                .iter()
                .filter(|c| if horizontal { c.x == edge } else { c.y == edge })
                .map(|c| if horizontal { c.y } else { c.x })
                .collect();
            let middle = across
                .iter()
                .min()
                .copied()
                .unwrap_or(0)
                .saturating_add(across.iter().max().copied().unwrap_or(0))
                / 2;
            if horizontal {
                CellPos::new(edge, middle)
            } else {
                CellPos::new(middle, edge)
            }
        });
        for (end_index, end) in ends.into_iter().enumerate() {
            let outward = |cell: CellPos| match (horizontal, end_index) {
                (true, 0) => cell.x < min_x,
                (true, _) => cell.x > max_x,
                (false, 0) => cell.y < min_y,
                (false, _) => cell.y > max_y,
            };
            let existing =
                world
                    .query::<(&SimId, &Building)>()
                    .iter(world)
                    .any(|(hut, building)| {
                        b.huts.contains(hut)
                            && outward(building.footprint.origin)
                            && building.footprint.distance_to_cell(end) <= 3
                    });
            if existing {
                continue;
            }
            let mut candidates = Vec::new();
            for y in end.y.saturating_sub(3)..=end.y.saturating_add(3) {
                for x in end.x.saturating_sub(3)..=end.x.saturating_add(3) {
                    let c = CellPos::new(x, y);
                    let grid = world.resource::<Grid>();
                    let distance = c.x.abs_diff(end.x).max(c.y.abs_diff(end.y));
                    let side = if horizontal { y != end.y } else { x != end.x };
                    if !side
                        || !outward(c)
                        || distance == 0
                        || !grid.cell(c).is_some_and(|c| {
                            (c.passable || (c.water && distance <= 2)) && c.ore == 0
                        })
                        || world.resource::<Props>().bridge_anchor(c).is_some()
                        || grid.building(c).is_some()
                        || grid.holders(c).iter().any(Option::is_some)
                    {
                        continue;
                    }
                    let water = grid.cell(c).is_some_and(|c| c.water);
                    let neighbours = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                        .iter()
                        .filter(|&&(dx, dy)| {
                            grid.cell(CellPos::new(x + dx, y + dy))
                                .is_some_and(|n| n.passable || (water && n.water))
                        })
                        .count();
                    if neighbours >= 3 {
                        candidates.push((water, distance, c));
                    }
                }
            }
            candidates.sort_unstable();
            if let Some((_, _, cell)) = candidates.first()
                && let Some(hut) = crate::world_objects::place(world, "bridge_hut", *cell)
            {
                world.resource_mut::<Grid>().set_world_feature(*cell, 15);
                b.huts.insert(hut);
                state.huts.insert(hut, *id);
            }
        }
    }
    world.insert_resource(state);
}

pub(crate) fn validate_repair(world: &World, hut: SimId) -> Result<(), crate::RejectReason> {
    let state = world.resource::<Bridges>();
    let bridge = state
        .huts
        .get(&hut)
        .and_then(|id| state.bridges.get(id))
        .filter(|b| b.collapsed)
        .ok_or_else(|| crate::special::unavailable("This hut has no collapsed bridge"))?;
    let grid = world.resource::<Grid>();
    // A holder includes both its current cell and a reserved step destination. Keep ships
    // and the Engineer intact until restoration can preserve their navigation domain.
    if bridge.cells.iter().any(|&cell| {
        grid.holders_for(cell, crate::Domain::Land)
            .into_iter()
            .flatten()
            .any(|id| grid.domain_of(id) == crate::Domain::Naval)
    }) {
        return Err(crate::special::unavailable(
            "Ships must clear the collapsed bridge before repair",
        ));
    }
    Ok(())
}
pub(crate) fn repair(
    world: &mut World,
    engineer: &crate::special::Actor,
    hut: SimId,
) -> Result<(), crate::RejectReason> {
    validate_repair(world, hut)?;
    let state = world.resource::<Bridges>();
    let Some(&id) = state.huts.get(&hut) else {
        return Err(crate::RejectReason::UnknownTarget(hut));
    };
    let Some(bridge) = state.bridges.get(&id).cloned() else {
        return Err(crate::RejectReason::UnknownTarget(hut));
    };
    let hp = world
        .resource::<Definitions>()
        .rules()
        .map_or(1500, |r| r.combat.bridge_strength);
    for (&cell, &deck) in bridge.cells.iter().zip(&bridge.decks) {
        world.resource_mut::<Grid>().set_terrain(cell, deck);
    }
    world
        .resource_mut::<Props>()
        .restore_bridge(&bridge.cells, hp);
    if let Some(b) = world.resource_mut::<Bridges>().bridges.get_mut(&id) {
        b.collapsed = false;
    }
    crate::special::release(world, engineer);
    world.despawn(engineer.entity);
    world
        .resource_mut::<bevy_ecs::message::Messages<crate::SimEvent>>()
        .write(crate::SimEvent::BridgeChanged {
            bridge: id,
            cells: bridge.cells,
            intact: true,
        });
    Ok(())
}
pub(crate) fn collapse(world: &mut World) {
    let groups = world.resource::<Bridges>().bridges.clone();
    for (id, b) in groups {
        if b.collapsed
            || !world
                .resource::<Props>()
                .get(id)
                .is_some_and(|(_, p)| p.hit_points <= 0)
        {
            continue;
        }
        let cells: BTreeSet<_> = b.cells.iter().copied().collect();
        for &cell in &b.cells {
            world.resource_mut::<Grid>().set_terrain(
                cell,
                Cell {
                    passable: false,
                    water: true,
                    buildable: false,
                    ore: 0,
                },
            );
        }
        let drowned: Vec<_> = world
            .query::<(bevy_ecs::entity::Entity, &SimId, &crate::Mobile)>()
            .iter(world)
            .filter(|(_, id, m)| {
                world.resource::<Grid>().domain_of(**id) == crate::Domain::Land
                    && (cells.contains(&m.cell) || m.step.is_some_and(|s| cells.contains(&s.to)))
            })
            .map(|(e, _, _)| e)
            .collect();
        for entity in drowned {
            if let Some(mut hp) = world.get_mut::<crate::Health>(entity) {
                hp.hit_points = 0;
            }
        }
        if let Some(bridge) = world.resource_mut::<Bridges>().bridges.get_mut(&id) {
            bridge.collapsed = true;
        }
        world
            .resource_mut::<bevy_ecs::message::Messages<crate::SimEvent>>()
            .write(crate::SimEvent::BridgeChanged {
                bridge: id,
                cells: b.cells,
                intact: false,
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ra5_bridge_saved_decks_and_hut_associations_are_hashed() {
        let original = Bridges {
            bridges: BTreeMap::from([(
                SimId(1),
                Bridge {
                    cells: vec![CellPos::new(5, 5)],
                    collapsed: false,
                    huts: BTreeSet::from([SimId(2)]),
                    decks: vec![Cell {
                        passable: true,
                        water: false,
                        buildable: false,
                        ore: 0,
                    }],
                },
            )]),
            huts: BTreeMap::from([(SimId(2), SimId(1))]),
        };
        let hash = |s: &Bridges| {
            let mut h = crate::Fnv1a64::new();
            s.hash_into(&mut h);
            h.finish()
        };
        let base = hash(&original);
        for field in 0..7 {
            let mut changed = original.clone();
            let b = changed.bridges.get_mut(&SimId(1)).expect("bridge");
            match field {
                0 => b.cells[0].x += 1,
                1 => b.collapsed = true,
                2 => b.decks[0].passable = false,
                3 => b.decks[0].water = true,
                4 => b.decks[0].ore = 1,
                5 => {
                    b.huts.insert(SimId(3));
                }
                _ => {
                    changed.huts.insert(SimId(2), SimId(3));
                }
            }
            assert_ne!(hash(&changed), base, "field {field}");
        }
        assert_eq!(hash(&original), base);
    }
}
