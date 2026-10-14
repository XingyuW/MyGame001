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
    Bridge,
}

impl PropKind {
    /// The kind of the scenery feature of `look`, if it is destructible scenery.
    pub fn of_look(look: u8) -> Option<Self> {
        match look >> 4 {
            4 => Some(Self::Trees),
            5 => Some(Self::House),
            6 => Some(Self::CityBlock),
            7 => Some(Self::Tower),
            8 => Some(Self::Bridge),
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
            Self::Bridge => 1_500,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Trees => "Trees",
            Self::House => "House",
            Self::CityBlock => "City block",
            Self::Tower => "Tower",
            Self::Bridge => "Bridge",
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
    armours: BTreeMap<PropKind, ArmourClass>,
    bridge_of: BTreeMap<CellPos, CellPos>,
    pub(crate) bridge_destructible: bool,
    cells: BTreeMap<CellPos, Prop>,
    /// Cells whose prop was reduced to 0 hit points since the last `Cleanup` stage.
    dying: BTreeSet<CellPos>,
}

impl Props {
    /// Shared client/headless installation from the simulation map's retained features.
    pub fn from_grid(grid: &Grid, armour: ArmourClass) -> Self {
        let mut looks = vec![0; (u64::from(grid.width()) * u64::from(grid.height())) as usize];
        for (cell, feature) in grid.world_features() {
            let index = cell.y as usize * grid.width() as usize + cell.x as usize;
            if let Some(look) = looks.get_mut(index) {
                *look = feature << 4;
            }
        }
        Self::from_looks(grid, &looks, armour)
    }

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
            if grid.cell(cell).is_some_and(|c| {
                (!c.passable && !c.water) || (kind == PropKind::Bridge && c.passable)
            }) {
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
            armours: BTreeMap::new(),
            bridge_of: BTreeMap::new(),
            bridge_destructible: true,
            cells,
            dying: BTreeSet::new(),
        }
    }

    pub(crate) fn configure(&mut self, defs: &crate::Definitions) {
        for kind in [PropKind::House, PropKind::CityBlock, PropKind::Tower] {
            if let Some(crate::ProductionItem::Building(building)) =
                crate::garrison::item(defs, kind)
                && let Some(terms) = defs.building_combat(building)
            {
                self.armours.insert(kind, terms.armour);
                for prop in self.cells.values_mut().filter(|p| p.kind == kind) {
                    prop.hit_points = terms.hit_points;
                }
            }
        }
        if let Some(armour) = defs.armour_class("concrete") {
            self.armours.insert(PropKind::Bridge, armour);
        }
        let hp = defs.rules().map_or(1500, |r| r.combat.bridge_strength);
        let mut remaining: BTreeSet<_> = self
            .cells
            .iter()
            .filter(|(_, p)| p.kind == PropKind::Bridge)
            .map(|(&cell, _)| cell)
            .collect();
        while let Some(&anchor) = remaining.first() {
            remaining.remove(&anchor);
            let mut queue = std::collections::VecDeque::from([anchor]);
            while let Some(cell) = queue.pop_front() {
                self.bridge_of.insert(cell, anchor);
                if let Some(prop) = self.cells.get_mut(&cell) {
                    prop.hit_points = hp;
                }
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let next = CellPos::new(cell.x.saturating_add(dx), cell.y.saturating_add(dy));
                    if remaining.remove(&next) {
                        queue.push_back(next);
                    }
                }
            }
        }
    }
    pub fn armour_of(&self, kind: PropKind) -> ArmourClass {
        self.armours.get(&kind).copied().unwrap_or(self.armour)
    }
    pub(crate) fn bridge_anchor(&self, cell: CellPos) -> Option<CellPos> {
        self.bridge_of.get(&cell).copied()
    }
    pub(crate) fn bridge_cells(&self) -> impl Iterator<Item = (CellPos, CellPos)> + '_ {
        self.bridge_of.iter().map(|(&cell, &anchor)| (cell, anchor))
    }
    pub(crate) fn restore_bridge(&mut self, cells: &[CellPos], hp: i32) {
        for &cell in cells {
            self.cells.insert(
                cell,
                Prop {
                    kind: PropKind::Bridge,
                    hit_points: hp,
                },
            );
            self.dying.remove(&cell);
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
        if let Some(anchor) = self.bridge_anchor(cell) {
            if !self.bridge_destructible {
                return;
            }
            let hp = self.cells.get(&cell).map_or(0, |p| i64::from(p.hit_points));
            let left = i32::try_from(hp.saturating_sub(damage).max(0)).unwrap_or(0);
            let cells: Vec<_> = self
                .bridge_of
                .iter()
                .filter(|(_, group)| **group == anchor)
                .map(|(&cell, _)| cell)
                .collect();
            for cell in cells {
                if let Some(prop) = self.cells.get_mut(&cell) {
                    prop.hit_points = left;
                }
                if left == 0 {
                    self.dying.insert(cell);
                }
            }
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
        if !self.armours.is_empty() || !self.bridge_of.is_empty() {
            hasher.write(b"prop_types");
            hasher.write_u64(self.armours.len() as u64);
            for (kind, armour) in &self.armours {
                hasher.write(&[kind.tag()]);
                hasher.write(&armour.0.to_le_bytes());
            }
            hasher.write_u64(self.bridge_of.len() as u64);
            for (cell, anchor) in &self.bridge_of {
                for coordinate in [cell.x, cell.y, anchor.x, anchor.y] {
                    hasher.write(&coordinate.to_le_bytes());
                }
            }
            hasher.write(&[u8::from(self.bridge_destructible)]);
        }
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
                passable: prop.kind != PropKind::Bridge,
                water: prop.kind == PropKind::Bridge,
                buildable: prop.kind != PropKind::Bridge,
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
    fn ra5_bridge_health_damage_is_hashed() {
        let cell = CellPos::new(0, 0);
        let mut props = Props {
            width: 1,
            bridge_destructible: true,
            ..Props::default()
        };
        props.cells.insert(
            cell,
            Prop {
                kind: PropKind::Bridge,
                hit_points: 1500,
            },
        );
        props.bridge_of.insert(cell, cell);
        let hash = |p: &Props| {
            let mut h = Fnv1a64::new();
            p.hash_into(&mut h);
            h.finish()
        };
        let original = hash(&props);
        props.hurt(cell, 1);
        assert_eq!(props.cells[&cell].hit_points, 1499);
        assert_ne!(hash(&props), original);
        props.cells.get_mut(&cell).expect("deck").hit_points = 1500;
        assert_eq!(hash(&props), original);
    }

    #[test]
    fn ids_and_cells_round_trip_and_only_scenery_looks_are_props() {
        assert_eq!(PropKind::of_look(4 << 4 | 1), Some(PropKind::Trees));
        assert_eq!(PropKind::of_look(7 << 4), Some(PropKind::Tower));
        assert_eq!(PropKind::of_look(2 << 4), None, "cliffs stay");
        assert_eq!(PropKind::of_look(0), None);
        let props = Props {
            width: 10,
            armour: ArmourClass::default(),
            armours: BTreeMap::new(),
            bridge_of: BTreeMap::new(),
            bridge_destructible: true,
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
            armours: BTreeMap::new(),
            bridge_of: BTreeMap::new(),
            bridge_destructible: true,
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
