//! Destructible scenery: the trees, cottages, city blocks and towers of a map.
//!
//! A map marks scenery with the feature of a cell's `look` (the high four bits, see
//! `LoadedMap::looks`); such a cell is impassable terrain. Every such cell of a match is a
//! [`Prop`] with hit points. Props belong to nobody: no unit or defence fires at one on its own,
//! and the computer players ignore them. A player orders an attack on one with the ordinary
//! [`Command::Attack`](crate::Command::Attack), naming the prop's id, and area damage (bombs,
//! strategic weapons) hurts the props it covers. A prop whose hit points are ≤ 0 is removed in
//! the `Cleanup` stage: its cell becomes open, buildable ground
//! ([`SimEvent::PropDestroyed`](crate::SimEvent::PropDestroyed)).
//!
//! Prop ids are not allocated: the prop of the cell with row-major index `i` has the id
//! [`PROP_ID_BASE`]` + i`, far above every id the allocator hands out in a match, so adding
//! props to a map changes no unit's or building's id.

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::message::Messages;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::ResMut;

use crate::combat_defs::ArmourClass;
use crate::event::SimEvent;
use crate::hash::Fnv1a64;
use crate::id::SimId;
use crate::map::{Cell, CellPos, Grid};

/// The id of the prop in the cell with row-major index 0.
pub const PROP_ID_BASE: u32 = 0xC000_0000;

/// What a prop is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PropKind {
    Trees,
    House,
    CityBlock,
    Tower,
}

impl PropKind {
    /// The kind of the scenery feature of `look`, if it is destructible scenery.
    pub fn of_look(look: u8) -> Option<Self> {
        match look >> 4 {
            4 => Some(Self::Trees),
            5 => Some(Self::House),
            6 => Some(Self::CityBlock),
            7 => Some(Self::Tower),
            _ => None,
        }
    }

    /// Hit points of an undamaged prop.
    pub fn hit_points(self) -> i32 {
        match self {
            Self::Trees => 60,
            Self::House => 300,
            Self::CityBlock => 600,
            Self::Tower => 1_000,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Trees => "Trees",
            Self::House => "House",
            Self::CityBlock => "City block",
            Self::Tower => "Tower",
        }
    }

    fn tag(self) -> u8 {
        self as u8
    }
}

/// One cell of destructible scenery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prop {
    pub kind: PropKind,
    pub hit_points: i32,
}

/// The destructible scenery of the match, by cell.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct Props {
    width: u32,
    armour: ArmourClass,
    cells: BTreeMap<CellPos, Prop>,
    /// Cells whose prop was reduced to 0 hit points since the last `Cleanup` stage.
    dying: BTreeSet<CellPos>,
}

impl Props {
    /// The props of a map: every impassable land cell of `grid` whose look (row-major `looks`)
    /// has a scenery feature. They take damage as `armour` does.
    pub fn from_looks(grid: &Grid, looks: &[u8], armour: ArmourClass) -> Self {
        let width = grid.width();
        let mut cells = BTreeMap::new();
        for (index, &look) in looks.iter().enumerate() {
            let Some(kind) = PropKind::of_look(look) else {
                continue;
            };
            let Some(cell) = cell_at(width, index) else {
                continue;
            };
            if grid.cell(cell).is_some_and(|c| !c.passable && !c.water) {
                cells.insert(
                    cell,
                    Prop {
                        kind,
                        hit_points: kind.hit_points(),
                    },
                );
            }
        }
        Self {
            width,
            armour,
            cells,
            dying: BTreeSet::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// The armour class every prop takes damage as.
    pub fn armour(&self) -> ArmourClass {
        self.armour
    }

    /// The prop standing in `cell`.
    pub fn at(&self, cell: CellPos) -> Option<&Prop> {
        self.cells.get(&cell)
    }

    /// The id of the prop standing in `cell`.
    pub fn id_at(&self, cell: CellPos) -> Option<SimId> {
        self.cells.contains_key(&cell).then(|| self.id_of(cell))?
    }

    fn id_of(&self, cell: CellPos) -> Option<SimId> {
        let index = u32::try_from(cell.y)
            .ok()?
            .checked_mul(self.width)?
            .checked_add(u32::try_from(cell.x).ok()?)?;
        PROP_ID_BASE.checked_add(index).map(SimId)
    }

    /// The cell and prop with the id `id`.
    pub fn get(&self, id: SimId) -> Option<(CellPos, &Prop)> {
        let index = id.0.checked_sub(PROP_ID_BASE)?;
        let cell = cell_at(self.width, index as usize)?;
        self.cells.get(&cell).map(|prop| (cell, prop))
    }

    /// Every prop in cell order.
    pub fn iter(&self) -> impl Iterator<Item = (CellPos, &Prop)> {
        self.cells.iter().map(|(&cell, prop)| (cell, prop))
    }

    /// Takes `damage` hit points from the prop in `cell`, if there is one.
    pub(crate) fn hurt(&mut self, cell: CellPos, damage: i64) {
        if damage <= 0 {
            return;
        }
        if let Some(prop) = self.cells.get_mut(&cell) {
            let left = i64::from(prop.hit_points).saturating_sub(damage).max(0);
            prop.hit_points = i32::try_from(left).unwrap_or(0);
            if prop.hit_points == 0 {
                self.dying.insert(cell);
            }
        }
    }

    /// Nothing for a match without props, so that the state hashes of such matches are those of
    /// earlier versions.
    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        if self.cells.is_empty() {
            return;
        }
        hasher.write_u64(self.cells.len() as u64);
        for (cell, prop) in &self.cells {
            hasher.write(&cell.x.to_le_bytes());
            hasher.write(&cell.y.to_le_bytes());
            hasher.write(&[prop.kind.tag()]);
            hasher.write(&prop.hit_points.to_le_bytes());
        }
    }
}

fn cell_at(width: u32, index: usize) -> Option<CellPos> {
    if width == 0 {
        return None;
    }
    let index = u32::try_from(index).ok()?;
    Some(CellPos::new(
        i32::try_from(index % width).ok()?,
        i32::try_from(index / width).ok()?,
    ))
}

/// Removes every prop without hit points, in cell order, and opens its cell (the `Cleanup`
/// stage, after the dead units and buildings).
pub(crate) fn remove_dead_props(
    mut props: ResMut<Props>,
    mut grid: ResMut<Grid>,
    mut events: ResMut<Messages<SimEvent>>,
) {
    if props.dying.is_empty() {
        return;
    }
    for cell in std::mem::take(&mut props.dying) {
        let Some(id) = props.id_of(cell) else {
            continue;
        };
        let Some(prop) = props.cells.remove(&cell) else {
            continue;
        };
        grid.set_terrain(
            cell,
            Cell {
                passable: true,
                water: false,
                buildable: true,
                ore: 0,
            },
        );
        events.write(SimEvent::PropDestroyed {
            prop: id,
            cell,
            kind: prop.kind,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_cells_round_trip_and_only_scenery_looks_are_props() {
        assert_eq!(PropKind::of_look(4 << 4 | 1), Some(PropKind::Trees));
        assert_eq!(PropKind::of_look(7 << 4), Some(PropKind::Tower));
        assert_eq!(PropKind::of_look(2 << 4), None, "cliffs stay");
        assert_eq!(PropKind::of_look(0), None);
        let props = Props {
            width: 10,
            armour: ArmourClass::default(),
            cells: BTreeMap::from([(
                CellPos::new(3, 2),
                Prop {
                    kind: PropKind::House,
                    hit_points: 300,
                },
            )]),
            dying: BTreeSet::new(),
        };
        let id = props.id_at(CellPos::new(3, 2)).unwrap();
        assert_eq!(id, SimId(PROP_ID_BASE + 23));
        assert_eq!(props.get(id).unwrap().0, CellPos::new(3, 2));
        assert!(props.id_at(CellPos::new(4, 2)).is_none());
        assert!(props.get(SimId(5)).is_none());
    }

    #[test]
    fn damage_never_goes_below_zero_and_marks_the_prop() {
        let cell = CellPos::new(0, 0);
        let mut props = Props {
            width: 4,
            armour: ArmourClass::default(),
            cells: BTreeMap::from([(
                cell,
                Prop {
                    kind: PropKind::Trees,
                    hit_points: 60,
                },
            )]),
            dying: BTreeSet::new(),
        };
        props.hurt(cell, 20);
        assert_eq!(props.at(cell).unwrap().hit_points, 40);
        assert!(props.dying.is_empty());
        props.hurt(cell, i64::MAX);
        assert_eq!(props.at(cell).unwrap().hit_points, 0);
        assert!(props.dying.contains(&cell));
    }
}
