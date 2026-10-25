//! Maps: the [`Grid`] resource and the loader for the Tiled map format of docs/DECISIONS.md (D8,
//! D13).
//!
//! A map file is validated completely before any of it is used, and every problem is reported as
//! a [`MapError`] naming the file, the offending field, and the reason (invariant I8). Before
//! `tiled` parses anything, the raw file is checked against the supported subset of the format
//! (`map_subset`, invariant I11). The `game` crate hands a map to `bevy_ecs_tiled` only after it
//! has passed [`load_map`] and the game's own check of the tileset image.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use bevy_ecs::resource::Resource;
use tiled::{Loader, Orientation, Properties, PropertyValue, TileLayer};

use crate::hash::Fnv1a64;
use crate::id::SimId;
use crate::map_subset;

/// The map format version this crate reads (map property `schema_version`).
pub const MAP_SCHEMA_VERSION: i32 = 2;

/// Largest accepted map width and height, in cells.
pub const MAX_MAP_SIDE: u32 = 256;

/// Name of the one tile layer a map consists of.
pub const TERRAIN_LAYER: &str = "terrain";

/// Width of a cell's diamond in pixels: the only supported tile width. Exact picking is
/// established for this tile size (docs/DECISIONS.md, D9 and D13).
pub const TILE_WIDTH_PX: u32 = 64;

/// Height of a cell's diamond in pixels: the only supported tile height.
pub const TILE_HEIGHT_PX: u32 = 32;

/// Largest accepted `tilecount` of a tileset. `tiled` allocates an entry per tile id below
/// `tilecount` before anything else is checked (M1 review 1, P0-3), and the renderer copies every
/// tile into one layer of a texture array, whose layer count every GPU supports only up to 256
/// (`wgpu::Limits::default().max_texture_array_layers`; M1 review 2, P1-A; docs/DECISIONS.md D16).
pub const MAX_TILESET_TILES: u32 = 256;

/// Largest accepted width and height of a tileset image in pixels. The renderer uploads the image
/// as one 2D texture, and every GPU supports a texture side of 8192 pixels
/// (`wgpu::Limits::default().max_texture_dimension_2d`; M1 review 2, P1-A; D16).
pub const MAX_TILESET_IMAGE_SIDE_PX: u32 = 8192;

/// Largest accepted size of a map or tileset file in bytes. A 256×256 map whose cells all carry
/// flip flags needs about 0.75 MB.
pub const MAX_MAP_FILE_BYTES: u64 = 4 * 1024 * 1024;

/// A map cell. Cell (0, 0) is the top corner of the isometric diamond; +x runs down-right and +y
/// runs down-left on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellPos {
    pub x: i32,
    pub y: i32,
}

impl CellPos {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// Subcells per cell along each axis (PLAN.md section 5).
pub const SUBCELLS_PER_CELL: i32 = 256;

/// A fine position in subcells. Subcell s lies in cell `s.div_euclid(256)`, and the centre of
/// cell c is at `c * 256 + 128` (PLAN.md section 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SubPos {
    pub x: i32,
    pub y: i32,
}

impl SubPos {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// The centre of `cell`. Saturates instead of overflowing for cells far outside any map
    /// (invariant I12); inside a map (at most 256 cells per side) it is exact.
    pub const fn centre_of(cell: CellPos) -> Self {
        const HALF: i32 = SUBCELLS_PER_CELL / 2;
        Self {
            x: cell
                .x
                .saturating_mul(SUBCELLS_PER_CELL)
                .saturating_add(HALF),
            y: cell
                .y
                .saturating_mul(SUBCELLS_PER_CELL)
                .saturating_add(HALF),
        }
    }

    /// Where an infantry unit in spot `spot` of `cell` stands: the centre of the cell moved by
    /// [`SPOT_OFFSETS`] (a spot beyond the last counts from the first again).
    pub const fn spot_of(cell: CellPos, spot: u8) -> Self {
        let centre = Self::centre_of(cell);
        let (dx, dy) = SPOT_OFFSETS[spot as usize % SPOTS_PER_CELL];
        Self {
            x: centre.x.saturating_add(dx),
            y: centre.y.saturating_add(dy),
        }
    }

    /// The cell that contains this position.
    pub const fn cell(self) -> CellPos {
        CellPos {
            x: self.x.div_euclid(SUBCELLS_PER_CELL),
            y: self.y.div_euclid(SUBCELLS_PER_CELL),
        }
    }
}

/// How many infantry a surface cell holds at most, each in a spot of its own (RA3,
/// docs/DECISIONS.md R6).
pub const SPOTS_PER_CELL: usize = 3;

/// The offset of each infantry spot from the centre of its cell, in subcells. On screen (+x runs
/// down-right, +y down-left) spot 0 lies above the centre and spots 1 and 2 below it to the
/// right and to the left, mirror images of each other; the three offsets add up to nothing, so
/// three soldiers stand around the centre, and every spot lies well inside its cell.
pub const SPOT_OFFSETS: [(i32, i32); SPOTS_PER_CELL] = [(-64, -64), (80, -16), (-16, 80)];

/// The room of a free cell, counted in spots ([`Grid::room`]): a vehicle, ship or aircraft needs
/// the whole cell, an infantry unit one spot.
pub const CELL_ROOM: u8 = SPOTS_PER_CELL as u8;

/// No unit, in [`Grid`]'s table of spots: the id the allocator never hands out.
const NO_UNIT: u32 = u32::MAX;

/// Terrain of one cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    /// Ground units may enter the cell.
    pub passable: bool,
    /// Explicit water terrain; rocks are never navigable by ships.
    pub water: bool,
    /// Buildings may cover the cell.
    pub buildable: bool,
    /// Ore stored in the cell, in ore units: bails since RA5 (docs/RA2_PARITY_PLAN.md section
    /// 4.10). Whether they are bails of ore or of gems is the grid's ([`Grid::is_gems`]).
    pub ore: u32,
}

/// Per cell terrain and occupant of the current map (PLAN.md section 5).
///
/// [`SimPlugin`](crate::SimPlugin) starts with an empty 0×0 grid; match setup replaces it with
/// the grid of a [`LoadedMap`] before the first tick.
///
/// The occupant of a cell is the unit that stands in it or has reserved it for entry (invariant
/// I6: at most one ground unit per cell). Aircraft hold cells in a separate air layer, so they fly
/// over ground and naval units (and those drive under them) without either blocking the other;
/// at most one aircraft holds a cell of the air layer. Only the simulation's movement and
/// spawning change occupants.
///
/// Since RA3 the invariant is I6' (docs/DECISIONS.md R6): a surface cell holds at most one
/// vehicle (or ship) or at most [`SPOTS_PER_CELL`] infantry, and never both. A unit registered
/// as on foot ([`Grid::is_on_foot`]: a unit of a type with the arsenal's `infantry` trait that
/// does not fly) holds one spot of each cell it holds, and stands at that spot
/// ([`SubPos::spot_of`]); every other unit holds the whole cell, as before. The grid is the one
/// place that decides whether a unit may enter a cell ([`Grid::claim`], [`Grid::room`]): a
/// world without units on foot, which is every world without that trait, has no spots at all.
///
/// A cell covered by a building is impassable for units, whatever its terrain, so no unit can
/// reserve it (invariant I6: no unit occupies a cell covered by a building). Buildings cover only
/// cells that no unit holds when they are placed (docs/DECISIONS.md D24).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct Grid {
    width: u32,
    height: u32,
    pub(crate) navigation: crate::Domain,
    pub(crate) domains: BTreeMap<SimId, crate::Domain>,
    /// Row-major: the cell (x, y) is at index `y * width + x`.
    cells: Vec<Cell>,
    /// Row-major, as `cells`: the vehicle or ship holding the cell.
    occupants: Vec<Option<SimId>>,
    /// Row-major, as `cells`: the id of the infantry unit holding each spot of the cell
    /// (`NO_UNIT`: none). Empty until the first unit on foot is registered, so a world without
    /// infantry on foot pays nothing for it.
    spots: Vec<[u32; SPOTS_PER_CELL]>,
    /// The units on foot: those that hold a spot of a cell instead of the cell.
    foot: BTreeSet<SimId>,
    /// Row-major, as `cells`: the aircraft holding the cell.
    air_occupants: Vec<Option<SimId>>,
    /// Row-major, as `cells`: the building covering the cell.
    buildings: Vec<Option<SimId>>,
    /// The row-major indices of the cells whose ore is gems (RA5): always cells with ore. Empty
    /// on a map without gems, which is hashed as before RA5.
    gems: BTreeSet<u32>,
    /// Semantic map objects, retained when headless callers discard presentation looks.
    world_features: BTreeMap<CellPos, u8>,
    /// The row-major indices of the cells a harvester emptied and that hold no ore now (RA5):
    /// ground ore may spread onto again, though a map's ore ground is no building ground.
    mined: BTreeSet<u32>,
}

impl Grid {
    /// A grid of `width` × `height` cells with the terrain `cells` (row-major: the cell (x, y) is
    /// at index `y * width + x`), without units or buildings, for land navigation: what
    /// [`load_map`] makes of a map file. `None` if a side exceeds [`MAX_MAP_SIDE`] or `cells` does
    /// not hold exactly one cell per cell of the grid. Scenario setup only, like replacing the
    /// whole grid (`crate::map_scale` builds the enlarged map of a match with it).
    pub fn from_cells(width: u32, height: u32, cells: Vec<Cell>) -> Option<Self> {
        let count = usize::try_from(u64::from(width) * u64::from(height)).ok()?;
        if width > MAX_MAP_SIDE || height > MAX_MAP_SIDE || cells.len() != count {
            return None;
        }
        Some(Self {
            width,
            height,
            navigation: crate::Domain::Land,
            domains: BTreeMap::new(),
            occupants: vec![None; count],
            spots: Vec::new(),
            foot: BTreeSet::new(),
            air_occupants: vec![None; count],
            buildings: vec![None; count],
            gems: BTreeSet::new(),
            world_features: BTreeMap::new(),
            mined: BTreeSet::new(),
            cells,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    /// Scenario setup: retain only semantic scenery, bridge and neutral-building features.
    /// Presentation ground colours stay outside the simulation.
    pub fn mark_world_features(&mut self, looks: &[u8]) {
        self.world_features.clear();
        for (index, &look) in looks.iter().take(self.cells.len()).enumerate() {
            let feature = look >> 4;
            if matches!(feature, 4..=8 | 11..=15) && self.width > 0 {
                let index = index as u32;
                self.world_features.insert(
                    CellPos::new((index % self.width) as i32, (index / self.width) as i32),
                    feature,
                );
            }
        }
    }

    /// Semantic source anchors and deck cells, in deterministic cell order.
    pub fn world_features(&self) -> impl Iterator<Item = (CellPos, u8)> + '_ {
        self.world_features
            .iter()
            .map(|(&cell, &feature)| (cell, feature))
    }

    pub(crate) fn set_world_feature(&mut self, cell: CellPos, feature: u8) {
        if self.contains(cell) && matches!(feature, 4..=8 | 11..=15) {
            self.world_features.insert(cell, feature);
        } else {
            self.world_features.remove(&cell);
        }
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn contains(&self, pos: CellPos) -> bool {
        self.index(pos).is_some()
    }

    /// The cell at `pos`, or `None` outside the map.
    pub fn cell(&self, pos: CellPos) -> Option<&Cell> {
        self.index(pos).and_then(|index| self.cells.get(index))
    }

    /// Whether ground units may enter `pos`: its terrain is passable and no building covers it;
    /// `false` outside the map.
    pub fn is_passable(&self, pos: CellPos) -> bool {
        self.passable_for(pos, self.navigation)
    }

    pub fn passable_for(&self, pos: CellPos, domain: crate::Domain) -> bool {
        self.cell(pos).is_some_and(|cell| match domain {
            crate::Domain::Land => cell.passable,
            crate::Domain::Naval => cell.water,
            crate::Domain::Amphibious => cell.passable || cell.water,
            crate::Domain::Air => true,
        }) && (domain == crate::Domain::Air || self.building(pos).is_none())
    }
    pub fn domain_of(&self, id: SimId) -> crate::Domain {
        self.domains.get(&id).copied().unwrap_or_default()
    }
    /// Whether `id` is a unit on foot: it holds one spot of a surface cell, not the cell.
    pub fn is_on_foot(&self, id: SimId) -> bool {
        self.foot.contains(&id)
    }
    /// Records the domain of the new unit `id` and whether it is on foot (an infantry unit that
    /// does not fly), before it claims its first cell.
    pub(crate) fn register(&mut self, id: SimId, domain: crate::Domain, on_foot: bool) {
        if domain != crate::Domain::Land {
            self.domains.insert(id, domain);
        }
        if on_foot && domain != crate::Domain::Air {
            self.foot.insert(id);
            if self.spots.is_empty() {
                self.spots = vec![[NO_UNIT; SPOTS_PER_CELL]; self.cells.len()];
            }
        }
    }
    /// Forgets the unit `id`, which is gone; its cells are released separately.
    pub(crate) fn unregister(&mut self, id: SimId) {
        self.domains.remove(&id);
        self.foot.remove(&id);
    }
    /// Navigation view reuses the existing deterministic A* and destination allocator.
    pub fn navigation_for(&self, domain: crate::Domain) -> std::borrow::Cow<'_, Self> {
        if domain == self.navigation {
            std::borrow::Cow::Borrowed(self)
        } else {
            let mut copy = self.clone();
            copy.navigation = domain;
            std::borrow::Cow::Owned(copy)
        }
    }

    /// The building covering `pos`, if any.
    pub fn building(&self, pos: CellPos) -> Option<SimId> {
        self.index(pos)
            .and_then(|index| self.buildings.get(index))
            .copied()
            .flatten()
    }

    /// Marks `pos` as covered by `building`. The caller has checked that the cell is inside the
    /// map, free of units, and not covered (`building::check_footprint`).
    pub(crate) fn cover(&mut self, pos: CellPos, building: SimId) {
        if let Some(slot) = self
            .index(pos)
            .and_then(|index| self.buildings.get_mut(index))
        {
            *slot = Some(building);
        }
    }

    /// Frees `pos` if `building` covers it.
    pub(crate) fn uncover(&mut self, pos: CellPos, building: SimId) {
        if let Some(slot) = self
            .index(pos)
            .and_then(|index| self.buildings.get_mut(index))
            && *slot == Some(building)
        {
            *slot = None;
        }
    }

    /// Removes up to `max` ore units from `pos` and returns how many were removed: the smaller of
    /// `max` and the ore in the cell, so the ore never falls below zero (PLAN.md section 5,
    /// equation 4). A cell of gems that is emptied is a cell of gems no longer.
    pub(crate) fn take_ore(&mut self, pos: CellPos, max: u32) -> u32 {
        let Some(index) = self.index(pos) else {
            return 0;
        };
        match self.cells.get_mut(index) {
            Some(cell) => {
                let taken = cell.ore.min(max);
                cell.ore -= taken;
                if taken > 0 && cell.ore == 0 {
                    self.gems.remove(&(index as u32));
                    self.mined.insert(index as u32);
                }
                taken
            }
            None => 0,
        }
    }

    /// Whether the ore in `pos` is gems (RA5): `false` for a cell of ore, a cell without ore
    /// and a cell outside the map.
    pub fn is_gems(&self, pos: CellPos) -> bool {
        self.index(pos)
            .is_some_and(|index| self.gems.contains(&(index as u32)))
    }

    /// How many cells hold gems.
    pub fn gem_cells(&self) -> usize {
        self.gems.len()
    }

    /// Makes the ore in `pos` gems (`gems`) or ore. Returns whether the cell is now as asked:
    /// `false` outside the map and for gems in a cell without ore. Scenario setup only, like
    /// [`Grid::set_terrain`].
    pub fn set_gems(&mut self, pos: CellPos, gems: bool) -> bool {
        let Some(index) = self.index(pos) else {
            return false;
        };
        if !gems {
            self.gems.remove(&(index as u32));
            return true;
        }
        if self.cells.get(index).is_none_or(|cell| cell.ore == 0) {
            return false;
        }
        self.gems.insert(index as u32);
        true
    }

    /// Marks as gems every cell with ore whose look (`looks`, row-major) has the feature
    /// [`LOOK_FEATURE_GEMS`](crate::LOOK_FEATURE_GEMS): how a map says where its gems are.
    pub(crate) fn mark_gems(&mut self, looks: &[u8]) {
        for (index, (cell, &look)) in self.cells.iter().zip(looks).enumerate() {
            if cell.ore > 0 && look >> 4 == crate::map_scale::LOOK_FEATURE_GEMS {
                self.gems.insert(index as u32);
            }
        }
    }

    /// Turns the ore amounts of a map file into bails (RA5, docs/RA2_PARITY_PLAN.md section
    /// 4.10): a cell with the amount `a` holds `round(a / value)` bails, at least 1 and at most
    /// `max_bails`, where `value` is `gem_value` for a cell of gems and `ore_value` for any
    /// other (a value below 1 counts as 1). Match setup only: once, before the first tick.
    pub(crate) fn ore_to_bails(&mut self, ore_value: u32, gem_value: u32, max_bails: u32) {
        for (index, cell) in self.cells.iter_mut().enumerate() {
            if cell.ore == 0 {
                continue;
            }
            let value = if self.gems.contains(&(index as u32)) {
                gem_value
            } else {
                ore_value
            }
            .max(1);
            // Half away from zero: round(a / b) = (2a + b) / (2b), in `u64` (invariant I12).
            let bails = (2 * u64::from(cell.ore) + u64::from(value)) / (2 * u64::from(value));
            cell.ore = u32::try_from(bails)
                .unwrap_or(u32::MAX)
                .clamp(1, max_bails.max(1));
        }
    }

    /// Adds one bail to the ore in `pos` if the cell holds ore (not gems) below `max_bails`.
    /// Returns whether it grew (RA5: ore growth, `crate::ore`).
    pub(crate) fn grow_ore(&mut self, pos: CellPos, max_bails: u32) -> bool {
        let Some(index) = self.index(pos) else {
            return false;
        };
        if self.gems.contains(&(index as u32)) {
            return false;
        }
        match self.cells.get_mut(index) {
            Some(cell) if cell.ore > 0 && cell.ore < max_bails => {
                cell.ore += 1;
                true
            }
            _ => false,
        }
    }

    /// Whether ore may spread onto `pos` (RA5, `crate::ore`): a land cell inside the map that
    /// ground units may enter, that holds no ore and no building, and that is building ground
    /// or was emptied by a harvester (a map's ore ground is no building ground, and neither
    /// are its roads, beaches and bridges, which stay free of ore).
    pub fn takes_ore(&self, pos: CellPos) -> bool {
        let Some(index) = self.index(pos) else {
            return false;
        };
        self.cells.get(index).is_some_and(|cell| {
            cell.passable
                && !cell.water
                && cell.ore == 0
                && (cell.buildable || self.mined.contains(&(index as u32)))
        }) && self.buildings.get(index).is_some_and(Option::is_none)
    }

    /// Puts one bail of ore into `pos` if ore may spread onto it ([`Grid::takes_ore`]).
    pub(crate) fn plant_ore(&mut self, pos: CellPos) -> bool {
        if !self.takes_ore(pos) {
            return false;
        }
        let Some(index) = self.index(pos) else {
            return false;
        };
        match self.cells.get_mut(index) {
            Some(cell) => {
                cell.ore = 1;
                self.mined.remove(&(index as u32));
                true
            }
            None => false,
        }
    }

    /// The cells with ore that is not gems, with the bails each holds, in row-major order
    /// (RA5: the cells that grow and spread, `crate::ore`).
    pub(crate) fn ore_cells(&self) -> Vec<(CellPos, u32)> {
        let width = self.width.max(1) as usize;
        self.cells
            .iter()
            .enumerate()
            .filter(|(index, cell)| cell.ore > 0 && !self.gems.contains(&(*index as u32)))
            .map(|(index, cell)| {
                (
                    CellPos::new((index % width) as i32, (index / width) as i32),
                    cell.ore,
                )
            })
            .collect()
    }

    /// A unit standing in or entering `pos`, if any, in the occupancy layer of this view's
    /// navigation domain: the aircraft for an air view; otherwise the vehicle or ship, or of the
    /// infantry in the cell the one in the first held spot. `None` means that nothing holds the
    /// cell; whether a particular unit may enter a held cell is answered by [`Grid::room`].
    pub fn occupant(&self, pos: CellPos) -> Option<SimId> {
        self.occupant_for(pos, self.navigation)
    }

    /// A unit holding `pos` in the occupancy layer that units of `domain` move in (see
    /// [`Grid::occupant`]).
    pub fn occupant_for(&self, pos: CellPos, domain: crate::Domain) -> Option<SimId> {
        self.holders_for(pos, domain).into_iter().flatten().next()
    }

    /// Every unit standing in or entering `pos` in the occupancy layer of this view's navigation
    /// domain: the aircraft; or the vehicle or ship; or the infantry, each at the index of its
    /// spot.
    pub fn holders(&self, pos: CellPos) -> [Option<SimId>; SPOTS_PER_CELL] {
        self.holders_for(pos, self.navigation)
    }

    /// Every unit holding `pos` in the occupancy layer that units of `domain` move in (see
    /// [`Grid::holders`]).
    pub fn holders_for(
        &self,
        pos: CellPos,
        domain: crate::Domain,
    ) -> [Option<SimId>; SPOTS_PER_CELL] {
        let mut holders = [None; SPOTS_PER_CELL];
        let Some(index) = self.index(pos) else {
            return holders;
        };
        if domain == crate::Domain::Air {
            holders[0] = self.air_occupants.get(index).copied().flatten();
        } else if let Some(vehicle) = self.occupants.get(index).copied().flatten() {
            holders[0] = Some(vehicle);
        } else if let Some(spots) = self.spots.get(index) {
            for (holder, &spot) in holders.iter_mut().zip(spots) {
                *holder = (spot != NO_UNIT).then_some(SimId(spot));
            }
        }
        holders
    }

    /// The spot of the surface cell `pos` that the unit on foot `unit` holds, if it holds one.
    pub fn spot(&self, pos: CellPos, unit: SimId) -> Option<u8> {
        let spots = self.spots.get(self.index(pos)?)?;
        let spot = spots.iter().position(|&holder| holder == unit.0)?;
        u8::try_from(spot).ok()
    }

    /// Where `unit` stands when it is in `pos`: at the spot it holds there, or at the centre of
    /// the cell if it holds none (a vehicle, a ship, an aircraft).
    pub fn standing_point(&self, pos: CellPos, unit: SimId) -> SubPos {
        match self.spot(pos, unit) {
            Some(spot) => SubPos::spot_of(pos, spot),
            None => SubPos::centre_of(pos),
        }
    }

    /// The room a unit needs, in spots: one spot on foot, else the whole cell ([`CELL_ROOM`]).
    pub const fn room_needed(on_foot: bool) -> u8 {
        if on_foot { 1 } else { CELL_ROOM }
    }

    /// How much room `pos` has, in spots, for a unit on foot (`on_foot`) or any other unit, in
    /// the occupancy layer of this view's navigation domain, if of the units that hold the cell
    /// only those for which `stays` holds remained (invariant I6'): for a unit on foot nothing
    /// beside a vehicle or ship that stays, else a spot for every spot that no infantry unit
    /// that stays holds; for any other unit the whole cell ([`CELL_ROOM`]) if no holder stays,
    /// else nothing. Nothing outside the map. Terrain and buildings are not looked at
    /// ([`Grid::passable_for`]).
    pub fn room(&self, pos: CellPos, on_foot: bool, stays: impl Fn(SimId) -> bool) -> u8 {
        self.room_in(pos, self.navigation, on_foot, stays)
    }

    /// [`Grid::room`] in the occupancy layer that units of `domain` move in.
    pub fn room_in(
        &self,
        pos: CellPos,
        domain: crate::Domain,
        on_foot: bool,
        stays: impl Fn(SimId) -> bool,
    ) -> u8 {
        let Some(index) = self.index(pos) else {
            return 0;
        };
        let held = |holder: &Option<SimId>| holder.is_some_and(&stays);
        if domain == crate::Domain::Air {
            return if self.air_occupants.get(index).is_some_and(held) {
                0
            } else {
                CELL_ROOM
            };
        }
        if self.occupants.get(index).is_some_and(held) {
            return 0;
        }
        let taken = self.spots.get(index).map_or(0, |spots| {
            spots
                .iter()
                .filter(|&&spot| spot != NO_UNIT && stays(SimId(spot)))
                .count()
        });
        match (on_foot, u8::try_from(taken)) {
            (true, Ok(taken)) => CELL_ROOM.saturating_sub(taken),
            (false, Ok(0)) => CELL_ROOM,
            _ => 0,
        }
    }

    /// Whether `pos` has room for one more unit on foot (`on_foot`) or of any other kind, in the
    /// occupancy layer of this view's navigation domain: nothing holds it, or for a unit on foot
    /// no vehicle holds it and one of its spots is free.
    pub fn has_room(&self, pos: CellPos, on_foot: bool) -> bool {
        self.room(pos, on_foot, |_| true) >= Self::room_needed(on_foot)
    }

    /// Whether the units for which `stays` holds leave no room in `pos` for `unit`, which is on
    /// foot or not (`on_foot`), in the occupancy layer of this view's navigation domain. `unit`
    /// itself never keeps `unit` out.
    pub fn blocked(
        &self,
        pos: CellPos,
        unit: SimId,
        on_foot: bool,
        stays: impl Fn(SimId) -> bool,
    ) -> bool {
        self.room(pos, on_foot, |holder| holder != unit && stays(holder))
            < Self::room_needed(on_foot)
    }

    /// The units that keep `unit` (on foot or not) out of `pos`, in the occupancy layer of this
    /// view's navigation domain: none if it has room there or holds it already, else every
    /// other unit that holds the cell.
    pub fn blockers(
        &self,
        pos: CellPos,
        unit: SimId,
        on_foot: bool,
    ) -> [Option<SimId>; SPOTS_PER_CELL] {
        if !self.blocked(pos, unit, on_foot, |_| true) {
            return [None; SPOTS_PER_CELL];
        }
        self.holders(pos)
            .map(|holder| holder.filter(|&holder| holder != unit))
    }

    /// Whether `unit` may hold `pos` as far as other units go, in the occupancy layer of its own
    /// domain: it holds the cell already, or the cell has room for it.
    pub fn room_for(&self, pos: CellPos, unit: SimId) -> bool {
        let on_foot = self.is_on_foot(unit);
        self.room_in(pos, self.domain_of(unit), on_foot, |holder| holder != unit)
            >= Self::room_needed(on_foot)
    }

    /// Replaces the terrain of `pos` and returns whether `pos` is inside the map.
    ///
    /// Scenario setup only (tests and match setup), like replacing the whole grid; during a match
    /// terrain does not change, and cells become impassable only by being covered by a building
    /// placed with a command (M3). A unit standing on a cell that becomes impassable may still
    /// leave it, and units whose path crosses it repath.
    ///
    /// The ore of the new terrain is ore, not gems, whatever the cell held ([`Grid::set_gems`]).
    pub fn set_terrain(&mut self, pos: CellPos, cell: Cell) -> bool {
        let Some(index) = self.index(pos) else {
            return false;
        };
        match self.cells.get_mut(index) {
            Some(slot) => {
                *slot = cell;
                self.gems.remove(&(index as u32));
                self.mined.remove(&(index as u32));
                true
            }
            None => false,
        }
    }

    /// Makes `unit` a holder of `pos` if the cell is inside the map, passable, and has room for
    /// it (or is already held by it). Returns whether `unit` now holds the cell. A unit on foot
    /// takes the first free spot.
    pub(crate) fn claim(&mut self, pos: CellPos, unit: SimId) -> bool {
        self.claim_spot(pos, unit, None)
    }

    /// [`Grid::claim`]; a unit on foot takes the spot `preferred` if that is free, else the
    /// first free one (a unit that walks from cell to cell keeps the spot it had, so that a
    /// group keeps its formation).
    pub(crate) fn claim_spot(&mut self, pos: CellPos, unit: SimId, preferred: Option<u8>) -> bool {
        let domain = self.domain_of(unit);
        if !self.passable_for(pos, domain) {
            return false;
        }
        let Some(index) = self.index(pos) else {
            return false;
        };
        let on_foot = self.is_on_foot(unit);
        if !self.room_for(pos, unit) {
            return false;
        }
        if on_foot {
            let Some(spots) = self.spots.get_mut(index) else {
                return false;
            };
            if spots.contains(&unit.0) {
                return true;
            }
            let free = |spot: usize| spots.get(spot).is_some_and(|&holder| holder == NO_UNIT);
            let Some(spot) = preferred
                .map(usize::from)
                .filter(|&spot| free(spot))
                .or_else(|| (0..SPOTS_PER_CELL).find(|&spot| free(spot)))
            else {
                return false;
            };
            spots[spot] = unit.0;
            return true;
        }
        let layer = if domain == crate::Domain::Air {
            &mut self.air_occupants
        } else {
            &mut self.occupants
        };
        match layer.get_mut(index) {
            Some(slot) => {
                *slot = Some(unit);
                true
            }
            None => false,
        }
    }

    /// Frees `pos` if `unit` holds it, in either layer (a unit's domain may already be
    /// forgotten when it is removed).
    pub(crate) fn release(&mut self, pos: CellPos, unit: SimId) {
        let Some(index) = self.index(pos) else {
            return;
        };
        for layer in [&mut self.occupants, &mut self.air_occupants] {
            if let Some(slot) = layer.get_mut(index)
                && *slot == Some(unit)
            {
                *slot = None;
            }
        }
        if let Some(spots) = self.spots.get_mut(index) {
            for spot in spots.iter_mut().filter(|spot| **spot == unit.0) {
                *spot = NO_UNIT;
            }
        }
    }

    /// Frees every cell `unit` holds, given that it holds `pos`: a unit holds the cell it stands
    /// in and at most one neighbour of it, the one it enters or leaves.
    pub(crate) fn release_around(&mut self, pos: CellPos, unit: SimId) {
        for dy in -1..=1_i32 {
            for dx in -1..=1_i32 {
                self.release(
                    CellPos::new(pos.x.saturating_add(dx), pos.y.saturating_add(dy)),
                    unit,
                );
            }
        }
    }

    fn index(&self, pos: CellPos) -> Option<usize> {
        let x = u32::try_from(pos.x).ok()?;
        let y = u32::try_from(pos.y).ok()?;
        if x >= self.width || y >= self.height {
            return None;
        }
        usize::try_from(u64::from(y) * u64::from(self.width) + u64::from(x)).ok()
    }

    /// Width, height, then every cell in row-major order as one flag byte (bit 0 passable
    /// terrain, bit 1 buildable, bit 2 occupied, bit 3 covered by a building, bit 4 water, and
    /// since RA3 bits 5 to 7 for the infantry spots 0 to 2 that are held), the ore amount, and
    /// the occupant's and the building's ids where set, then the ids of the infantry in the held
    /// spots (docs/DECISIONS.md D11, extended in M2 and M3, and R6); then the aircraft of the
    /// air layer and the units on foot. A grid without units on foot is hashed as before RA3.
    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        hasher.write_u64(self.domains.len() as u64);
        for (id, domain) in &self.domains {
            hasher.write_u32(id.0);
            hasher.write(&[*domain as u8]);
        }
        hasher.write_u32(self.width);
        hasher.write_u32(self.height);
        const NO_SPOTS: [u32; SPOTS_PER_CELL] = [NO_UNIT; SPOTS_PER_CELL];
        for (index, ((cell, occupant), building)) in self
            .cells
            .iter()
            .zip(&self.occupants)
            .zip(&self.buildings)
            .enumerate()
        {
            let spots = self.spots.get(index).unwrap_or(&NO_SPOTS);
            let held = |spot: usize| u8::from(spots[spot] != NO_UNIT);
            hasher.write(&[u8::from(cell.passable)
                | (u8::from(cell.water) << 4)
                | (u8::from(cell.buildable) << 1)
                | (u8::from(occupant.is_some()) << 2)
                | (u8::from(building.is_some()) << 3)
                | (held(0) << 5)
                | (held(1) << 6)
                | (held(2) << 7)]);
            hasher.write_u32(cell.ore);
            if let Some(id) = occupant {
                hasher.write_u32(id.0);
            }
            if let Some(id) = building {
                hasher.write_u32(id.0);
            }
            for &spot in spots.iter().filter(|&&spot| spot != NO_UNIT) {
                hasher.write_u32(spot);
            }
        }
        // With units on foot both lists are written with their lengths, like every other part
        // of the hash whose length varies; without any, the hash is what it was before RA3
        // (docs/DECISIONS.md R6), when the aircraft were the last and only list.
        let lengths = !self.foot.is_empty();
        if lengths {
            hasher.write_u64(self.air_occupants.iter().flatten().count() as u64);
        }
        for (index, occupant) in self.air_occupants.iter().enumerate() {
            if let Some(id) = occupant {
                hasher.write_u64(index as u64);
                hasher.write_u32(id.0);
            }
        }
        if lengths {
            hasher.write_u64(self.foot.len() as u64);
        }
        for id in &self.foot {
            hasher.write_u32(id.0);
        }
        if !self.world_features.is_empty() {
            hasher.write(b"world_features");
            hasher.write_u64(self.world_features.len() as u64);
            for (cell, feature) in &self.world_features {
                hasher.write(&cell.x.to_le_bytes());
                hasher.write(&cell.y.to_le_bytes());
                hasher.write(&[*feature]);
            }
        }
        // RA5: which cells of ore are gems and which cells were mined out, each under a tag
        // and only if there are any, so that a map without gems hashes as before.
        for (tag, cells) in [(&b"gems"[..], &self.gems), (&b"mined"[..], &self.mined)] {
            if !cells.is_empty() {
                hasher.write(tag);
                hasher.write_u64(cells.len() as u64);
                for index in cells {
                    hasher.write_u32(*index);
                }
            }
        }
    }
}

/// A validated map file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedMap {
    pub grid: Grid,
    /// Width of one cell's diamond in pixels. Presentation data, not used by the simulation.
    pub tile_width: u32,
    /// Height of one cell's diamond in pixels. Presentation data, not used by the simulation.
    pub tile_height: u32,
    /// The tileset image the renderer will load. Presentation data, not used by the simulation;
    /// the game checks it against the image file before rendering (docs/DECISIONS.md, D13).
    pub tileset_image: TilesetImage,
    /// The look of every cell, in row-major order: the optional tile property `look` (0 when the
    /// tile has none). Presentation data: it tells the renderer what stands on a cell (a
    /// forest, a city block, a bridge) whose terrain is the same to the simulation. The game
    /// defines and checks the values. The simulation reads the scenery of a match from it
    /// (`crate::props`) and, since RA5, which cells of ore hold gems (the feature
    /// [`LOOK_FEATURE_GEMS`](crate::LOOK_FEATURE_GEMS), already applied to `grid`).
    pub looks: Vec<u8>,
}

/// A tileset image: its file name, which names a file in the map's directory (D13), and the size
/// in pixels that the map declares for it. The name is kept relative so that a map loads to the
/// same value wherever its file lies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TilesetImage {
    pub file_name: String,
    pub width: u32,
    pub height: u32,
}

/// Why a map file was rejected. See [`MapErrorKind`] for the reasons.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapError {
    pub file: PathBuf,
    /// The attribute, property, layer, or cell at fault, e.g. `layer "terrain" cell (3, 4)`.
    pub field: String,
    pub kind: MapErrorKind,
}

impl fmt::Display for MapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // An error inside an external tileset names the `.tsx` file (M1 review 1, P1-3), which is
        // a tileset file, not a map (M1 review 2, P2-A item 4).
        let what = if self
            .file
            .extension()
            .is_some_and(|extension| extension == "tsx")
        {
            "tileset"
        } else {
            "map"
        };
        write!(
            f,
            "{what} {}: {}: {}",
            self.file.display(),
            self.field,
            self.kind
        )
    }
}

impl std::error::Error for MapError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MapErrorKind {
    /// The file could not be read.
    Read(String),
    /// The file is not a well formed Tiled map (message from the XML or Tiled parser).
    Parse(String),
    UnsupportedOrientation(String),
    Infinite,
    ZeroSize {
        width: u32,
        height: u32,
    },
    TooLarge {
        width: u32,
        height: u32,
    },
    ZeroTileSize {
        width: u32,
        height: u32,
    },
    UnsupportedSchemaVersion(i32),
    MissingProperty,
    UnknownProperty,
    WrongPropertyType {
        expected: &'static str,
        found: &'static str,
    },
    NegativeOre(i32),
    TilesetWithoutImage,
    MissingLayer,
    UnexpectedLayer,
    LayerSizeMismatch {
        width: u32,
        height: u32,
    },
    UnsupportedEncoding {
        encoding: Option<String>,
        compression: Option<String>,
    },
    TileCountMismatch {
        expected: u64,
        found: u64,
    },
    EmptyCell,
    /// The cell's tile has no definition (and therefore no properties) in its tileset.
    UndefinedTile {
        tileset: String,
        id: u32,
    },
    /// The file is larger than [`MAX_MAP_FILE_BYTES`].
    FileTooLarge {
        max_bytes: u64,
    },
    /// The element is not part of the supported subset of the format (D13).
    UnsupportedElement,
    /// The attribute is not part of the supported subset of the format (D13).
    UnsupportedAttribute,
    MissingAttribute,
    /// The element, attribute, or property may appear only once.
    Duplicate,
    /// The value is outside the supported subset of the format (D13).
    InvalidValue {
        found: String,
        expected: String,
    },
    /// `bevy_ecs_tiled`'s own scan for external files would find other files than the ones
    /// accepted by the check (D13).
    ExternalReferenceMismatch {
        found: Vec<String>,
        expected: Vec<String>,
    },
    /// A required element is missing (D13).
    MissingElement,
    /// The element must come before the named element (D13).
    OutOfOrder {
        before: String,
    },
}

impl fmt::Display for MapErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(f, "cannot read the file: {error}"),
            Self::Parse(error) => write!(f, "not a valid Tiled map: {error}"),
            Self::UnsupportedOrientation(found) => {
                write!(
                    f,
                    "orientation {found:?} is not supported, expected \"isometric\""
                )
            }
            Self::Infinite => write!(f, "infinite maps are not supported"),
            Self::ZeroSize { width, height } => {
                write!(f, "map size {width}x{height} has a zero side")
            }
            Self::TooLarge { width, height } => write!(
                f,
                "map size {width}x{height} exceeds the maximum side of {MAX_MAP_SIDE} cells"
            ),
            Self::ZeroTileSize { width, height } => {
                write!(f, "tile size {width}x{height} px has a zero side")
            }
            Self::UnsupportedSchemaVersion(found) => write!(
                f,
                "schema version {found} is not supported, expected {MAP_SCHEMA_VERSION}"
            ),
            Self::MissingProperty => write!(f, "required property is missing"),
            Self::UnknownProperty => write!(f, "unknown property"),
            Self::WrongPropertyType { expected, found } => {
                write!(f, "expected a {expected} value, found a {found} value")
            }
            Self::NegativeOre(value) => write!(f, "ore amount {value} is negative"),
            Self::TilesetWithoutImage => {
                write!(
                    f,
                    "tileset must use a single image (image collections are not supported)"
                )
            }
            Self::MissingLayer => write!(f, "the map has no tile layer named {TERRAIN_LAYER:?}"),
            Self::UnexpectedLayer => write!(
                f,
                "unexpected layer: a map has exactly one tile layer, named {TERRAIN_LAYER:?}"
            ),
            Self::LayerSizeMismatch { width, height } => {
                write!(f, "layer size {width}x{height} differs from the map size")
            }
            Self::UnsupportedEncoding {
                encoding,
                compression,
            } => write!(
                f,
                "layer data encoding {encoding:?} with compression {compression:?} is not \
                 supported, expected uncompressed \"csv\""
            ),
            Self::TileCountMismatch { expected, found } => {
                write!(f, "layer data has {found} entries, expected {expected}")
            }
            Self::EmptyCell => write!(f, "cell has no tile"),
            Self::UndefinedTile { tileset, id } => {
                write!(
                    f,
                    "tile {id} of tileset {tileset:?} is not defined (it has no properties)"
                )
            }
            Self::FileTooLarge { max_bytes } => {
                write!(f, "the file is larger than {max_bytes} bytes")
            }
            Self::UnsupportedElement => write!(f, "element is not supported"),
            Self::UnsupportedAttribute => write!(f, "attribute is not supported"),
            Self::MissingAttribute => write!(f, "required attribute is missing"),
            Self::Duplicate => write!(f, "defined more than once"),
            Self::InvalidValue { found, expected } => {
                write!(f, "value {found:?} is not supported, expected {expected}")
            }
            Self::ExternalReferenceMismatch { found, expected } => write!(
                f,
                "the renderer's scan for external files finds {found:?}, expected {expected:?}: \
                 an external tileset must be referenced as source=\"name.tsx\", and no other \
                 text may contain source=\" or template=\" followed by a .tsx or .tx path"
            ),
            Self::MissingElement => write!(f, "required element is missing"),
            Self::OutOfOrder { before } => write!(f, "must come before {before}"),
        }
    }
}

/// Loads and validates the map file at `path`.
///
/// The file, and the external tileset it references if any, is first checked against the
/// supported subset of the Tiled format (`map_subset`, docs/DECISIONS.md D13) on its raw bytes;
/// only then does `tiled` parse it, from exactly the bytes that were checked. Malformed input,
/// including the hostile inputs of M1 review 1 (P0-1 to P0-4), is returned as a [`MapError`];
/// `tests/m1_map_subset.rs` covers them, and a seeded mutation test over the demo map checks
/// that no input it generates panics or exceeds a fixed memory budget.
pub fn load_map(path: impl AsRef<Path>) -> Result<LoadedMap, MapError> {
    load_map_with_files(path).map(|(map, _)| map)
}

/// A file that passed the subset check, and the exact bytes that were checked and parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckedFile {
    /// The path the file was read from: the map file, or its external tileset.
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}

/// [`load_map`], also returning the bytes of every file it checked and parsed (the map first,
/// then its external tileset if any), so that the renderer can be given exactly those bytes
/// instead of reading the files again (docs/DECISIONS.md, D16).
pub fn load_map_with_files(
    path: impl AsRef<Path>,
) -> Result<(LoadedMap, Vec<CheckedFile>), MapError> {
    let path = path.as_ref();
    let bytes = map_subset::read_file(path).map_err(|kind| MapError {
        file: path.to_owned(),
        field: "file".into(),
        kind,
    })?;
    let external = map_subset::check_map(path, &bytes)?;

    // `tiled` reads only what was checked: the map, and the external tileset at the path the
    // check resolved for it. Any other request fails.
    let reader = |requested: &Path| -> std::io::Result<Box<dyn Read>> {
        if requested == path {
            return Ok(Box::new(Cursor::new(bytes.clone())));
        }
        if let Some(tileset) = &external
            && requested == tileset.path
        {
            return Ok(Box::new(Cursor::new(tileset.bytes.clone())));
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{} is not part of the checked map", requested.display()),
        ))
    };
    let map = Loader::with_reader(reader)
        .load_tmx_map(path)
        .map_err(|e| MapError {
            file: path.to_owned(),
            field: "file".into(),
            kind: MapErrorKind::Parse(e.to_string()),
        })?;

    let loaded = validate(path, &map)?;
    let mut files = vec![CheckedFile {
        path: path.to_owned(),
        bytes,
    }];
    if let Some(tileset) = external {
        files.push(CheckedFile {
            path: tileset.path,
            bytes: tileset.bytes,
        });
    }
    Ok((loaded, files))
}

type Issue = (String, MapErrorKind);

/// The terrain and look of each tile, keyed by (tileset index, tile id).
type TileTerrain = BTreeMap<(usize, u32), (Cell, u8)>;

/// Checks the parsed map against the map format and builds the grid. Errors found in a tileset
/// name the file that contains it (the map, or the external tileset file).
fn validate(path: &Path, map: &tiled::Map) -> Result<LoadedMap, MapError> {
    let in_map = |(field, kind): Issue| MapError {
        file: path.to_owned(),
        field,
        kind,
    };
    check_header(map).map_err(in_map)?;
    check_map_properties(&map.properties).map_err(in_map)?;
    let terrain = tile_terrain(map)?;
    let layer = terrain_layer(map).map_err(in_map)?;
    let (cells, looks) = cells(map, &terrain, &layer).map_err(in_map)?;
    let tileset_image = tileset_image(map).map_err(in_map)?;
    let occupants = vec![None; cells.len()];
    let spots = Vec::new();
    let foot = BTreeSet::new();
    let air_occupants = vec![None; cells.len()];
    let buildings = vec![None; cells.len()];
    let mut grid = Grid {
        navigation: crate::Domain::Land,
        domains: BTreeMap::new(),
        width: map.width,
        height: map.height,
        cells,
        occupants,
        spots,
        foot,
        air_occupants,
        buildings,
        gems: BTreeSet::new(),
        world_features: BTreeMap::new(),
        mined: BTreeSet::new(),
    };
    // RA5: the one look the simulation reads from a map file, as it reads the ore amounts.
    grid.mark_gems(&looks);
    grid.mark_world_features(&looks);
    Ok(LoadedMap {
        grid,
        tile_width: map.tile_width,
        tile_height: map.tile_height,
        tileset_image,
        looks,
    })
}

/// Repeats the header checks of `map_subset` on the parsed result.
fn check_header(map: &tiled::Map) -> Result<(), Issue> {
    if map.orientation != Orientation::Isometric {
        return Err((
            "map attribute \"orientation\"".into(),
            MapErrorKind::UnsupportedOrientation(map.orientation.to_string()),
        ));
    }
    if map.infinite() {
        return Err(("map attribute \"infinite\"".into(), MapErrorKind::Infinite));
    }
    let (width, height) = (map.width, map.height);
    let size_field = || "map attributes \"width\" and \"height\"".to_string();
    if width == 0 || height == 0 {
        return Err((size_field(), MapErrorKind::ZeroSize { width, height }));
    }
    if width > MAX_MAP_SIDE || height > MAX_MAP_SIDE {
        return Err((size_field(), MapErrorKind::TooLarge { width, height }));
    }
    if map.tile_width == 0 || map.tile_height == 0 {
        return Err((
            "map attributes \"tilewidth\" and \"tileheight\"".into(),
            MapErrorKind::ZeroTileSize {
                width: map.tile_width,
                height: map.tile_height,
            },
        ));
    }

    Ok(())
}

/// The terrain and look of every cell, in row-major order.
fn cells(
    map: &tiled::Map,
    terrain: &TileTerrain,
    layer: &tiled::FiniteTileLayer<'_>,
) -> Result<(Vec<Cell>, Vec<u8>), Issue> {
    let (width, height) = (map.width, map.height);
    let mut cells = Vec::with_capacity(width as usize * height as usize);
    let mut looks = Vec::with_capacity(width as usize * height as usize);
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let field = || format!("layer {TERRAIN_LAYER:?} cell ({x}, {y})");
            let tile = layer
                .get_tile(x, y)
                .ok_or_else(|| (field(), MapErrorKind::EmptyCell))?;
            let (cell, look) =
                terrain
                    .get(&(tile.tileset_index(), tile.id()))
                    .ok_or_else(|| {
                        (
                            field(),
                            MapErrorKind::UndefinedTile {
                                tileset: tile.get_tileset().name.clone(),
                                id: tile.id(),
                            },
                        )
                    })?;
            cells.push(*cell);
            looks.push(*look);
        }
    }
    Ok((cells, looks))
}

/// The image of the map's tileset. `tiled` resolves the image path against the tileset's
/// directory, which is the map's directory (D13), so its file name identifies it.
fn tileset_image(map: &tiled::Map) -> Result<TilesetImage, Issue> {
    let missing = || ("tileset".to_string(), MapErrorKind::TilesetWithoutImage);
    let tileset = map.tilesets().first().ok_or_else(missing)?;
    let image = tileset.image.as_ref().ok_or_else(missing)?;
    let size = |name: &str, value: i32| {
        u32::try_from(value).map_err(|_| {
            (
                format!("tileset {:?} image attribute {name:?}", tileset.name),
                MapErrorKind::InvalidValue {
                    found: value.to_string(),
                    expected: "a positive 32-bit integer".into(),
                },
            )
        })
    };
    let file_name = image
        .source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(missing)?
        .to_owned();
    Ok(TilesetImage {
        file_name,
        width: size("width", image.width)?,
        height: size("height", image.height)?,
    })
}

/// Property names in sorted order, so that the reported error does not depend on hash map
/// iteration order (invariant I4).
fn sorted_names(properties: &Properties) -> Vec<&String> {
    let mut names: Vec<&String> = properties.keys().collect();
    names.sort();
    names
}

fn check_map_properties(properties: &Properties) -> Result<(), Issue> {
    let field = |name: &str| format!("map property {name:?}");
    match properties.get("schema_version") {
        None => return Err((field("schema_version"), MapErrorKind::MissingProperty)),
        Some(PropertyValue::IntValue(MAP_SCHEMA_VERSION)) => {}
        Some(PropertyValue::IntValue(found)) => {
            return Err((
                field("schema_version"),
                MapErrorKind::UnsupportedSchemaVersion(*found),
            ));
        }
        Some(other) => {
            return Err((
                field("schema_version"),
                MapErrorKind::WrongPropertyType {
                    expected: "int",
                    found: type_name(other),
                },
            ));
        }
    }
    match sorted_names(properties)
        .into_iter()
        .find(|name| *name != "schema_version")
    {
        Some(unknown) => Err((field(unknown), MapErrorKind::UnknownProperty)),
        None => Ok(()),
    }
}

/// Terrain and look of every tile defined in the map's tilesets, keyed by (tileset index, tile
/// id).
/// Errors name the file that holds the tileset (`tiled` records it as the tileset's `source`:
/// the external tileset file, or the map for an embedded tileset).
fn tile_terrain(map: &tiled::Map) -> Result<TileTerrain, MapError> {
    let mut terrain = BTreeMap::new();
    for (index, tileset) in map.tilesets().iter().enumerate() {
        let in_tileset = |(field, kind): Issue| MapError {
            file: tileset.source.clone(),
            field,
            kind,
        };
        let tileset_field = format!("tileset {:?}", tileset.name);
        if let Some(name) = sorted_names(&tileset.properties).first() {
            return Err(in_tileset((
                format!("{tileset_field} property {name:?}"),
                MapErrorKind::UnknownProperty,
            )));
        }
        if tileset.image.is_none() {
            return Err(in_tileset((
                tileset_field,
                MapErrorKind::TilesetWithoutImage,
            )));
        }
        let mut tiles: Vec<_> = tileset.tiles().collect();
        tiles.sort_by_key(|(id, _)| *id);
        for (id, tile) in tiles {
            let cell = parse_tile(&format!("{tileset_field} tile {id}"), &tile.properties)
                .map_err(in_tileset)?;
            terrain.insert((index, id), cell);
        }
    }
    Ok(terrain)
}

fn parse_tile(tile_field: &str, properties: &Properties) -> Result<(Cell, u8), Issue> {
    let field = |name: &str| format!("{tile_field} property {name:?}");
    if let Some(unknown) = sorted_names(properties).into_iter().find(|name| {
        !matches!(
            name.as_str(),
            "passable" | "buildable" | "ore" | "water" | "look"
        )
    }) {
        return Err((field(unknown), MapErrorKind::UnknownProperty));
    }
    let get = |name: &str| {
        properties
            .get(name)
            .ok_or_else(|| (field(name), MapErrorKind::MissingProperty))
    };
    let wrong_type = |name: &str, expected, found: &PropertyValue| {
        (
            field(name),
            MapErrorKind::WrongPropertyType {
                expected,
                found: type_name(found),
            },
        )
    };
    let bool_property = |name: &str| match get(name)? {
        PropertyValue::BoolValue(value) => Ok(*value),
        other => Err(wrong_type(name, "bool", other)),
    };

    let water = match properties.get("water") {
        None => false,
        Some(PropertyValue::BoolValue(v)) => *v,
        Some(other) => return Err(wrong_type("water", "bool", other)),
    };
    let passable = bool_property("passable")?;
    let buildable = bool_property("buildable")?;
    let ore = match get("ore")? {
        PropertyValue::IntValue(value) => {
            u32::try_from(*value).map_err(|_| (field("ore"), MapErrorKind::NegativeOre(*value)))?
        }
        other => return Err(wrong_type("ore", "int", other)),
    };
    // Optional and presentation only; the game gives the values their meaning.
    let look = match properties.get("look") {
        None => 0,
        Some(PropertyValue::IntValue(value)) => u8::try_from(*value).map_err(|_| {
            (
                field("look"),
                MapErrorKind::InvalidValue {
                    found: value.to_string(),
                    expected: "an integer from 0 to 255".into(),
                },
            )
        })?,
        Some(other) => return Err(wrong_type("look", "int", other)),
    };
    Ok((
        Cell {
            water,
            passable,
            buildable,
            ore,
        },
        look,
    ))
}

/// The map's only layer, which must be a finite tile layer named [`TERRAIN_LAYER`] with the
/// map's size and no properties.
fn terrain_layer(map: &tiled::Map) -> Result<tiled::FiniteTileLayer<'_>, Issue> {
    let mut found = None;
    for layer in map.layers() {
        let field = format!("layer {:?}", layer.name);
        let is_terrain = layer.name == TERRAIN_LAYER && found.is_none();
        match layer.as_tile_layer() {
            Some(TileLayer::Finite(tiles)) if is_terrain => {
                if let Some(name) = sorted_names(&layer.properties).first() {
                    return Err((
                        format!("{field} property {name:?}"),
                        MapErrorKind::UnknownProperty,
                    ));
                }
                if (tiles.width(), tiles.height()) != (map.width, map.height) {
                    return Err((
                        field,
                        MapErrorKind::LayerSizeMismatch {
                            width: tiles.width(),
                            height: tiles.height(),
                        },
                    ));
                }
                found = Some(tiles);
            }
            _ => return Err((field, MapErrorKind::UnexpectedLayer)),
        }
    }
    found.ok_or_else(|| {
        (
            format!("layer {TERRAIN_LAYER:?}"),
            MapErrorKind::MissingLayer,
        )
    })
}

fn type_name(value: &PropertyValue) -> &'static str {
    match value {
        PropertyValue::BoolValue(_) => "bool",
        PropertyValue::FloatValue(_) => "float",
        PropertyValue::IntValue(_) => "int",
        PropertyValue::ColorValue(_) => "color",
        PropertyValue::StringValue(_) => "string",
        PropertyValue::FileValue(_) => "file",
        PropertyValue::ObjectValue(_) => "object",
        PropertyValue::ListValue(_) => "list",
        PropertyValue::ClassValue { .. } => "class",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Domain;

    fn open(width: u32, height: u32) -> Grid {
        let cell = Cell {
            passable: true,
            water: false,
            buildable: true,
            ore: 0,
        };
        Grid::from_cells(width, height, vec![cell; (width * height) as usize]).expect("a grid")
    }

    fn hash(grid: &Grid) -> u64 {
        let mut hasher = Fnv1a64::new();
        grid.hash_into(&mut hasher);
        hasher.finish()
    }

    /// The hash of a grid as it was computed before RA3 (docs/DECISIONS.md D11, M2, M3, M9).
    fn hash_before_ra3(grid: &Grid) -> u64 {
        let mut hasher = Fnv1a64::new();
        hasher.write_u64(grid.domains.len() as u64);
        for (id, domain) in &grid.domains {
            hasher.write_u32(id.0);
            hasher.write(&[*domain as u8]);
        }
        hasher.write_u32(grid.width);
        hasher.write_u32(grid.height);
        for ((cell, occupant), building) in
            grid.cells.iter().zip(&grid.occupants).zip(&grid.buildings)
        {
            hasher.write(&[u8::from(cell.passable)
                | (u8::from(cell.water) << 4)
                | (u8::from(cell.buildable) << 1)
                | (u8::from(occupant.is_some()) << 2)
                | (u8::from(building.is_some()) << 3)]);
            hasher.write_u32(cell.ore);
            if let Some(id) = occupant {
                hasher.write_u32(id.0);
            }
            if let Some(id) = building {
                hasher.write_u32(id.0);
            }
        }
        for (index, occupant) in grid.air_occupants.iter().enumerate() {
            if let Some(id) = occupant {
                hasher.write_u64(index as u64);
                hasher.write_u32(id.0);
            }
        }
        hasher.finish()
    }

    /// RA3, invariant I6': the grid gives a surface cell to one vehicle or to three units on
    /// foot and never to both, and the air above it to one aircraft.
    #[test]
    fn a_cell_takes_one_vehicle_or_three_units_on_foot_and_never_both() {
        let mut grid = open(4, 4);
        let at = CellPos::new(1, 1);
        let (tank, truck) = (SimId(1), SimId(2));
        let soldiers = [SimId(10), SimId(11), SimId(12), SimId(13)];
        let plane = SimId(20);
        grid.register(tank, Domain::Land, false);
        grid.register(truck, Domain::Land, false);
        for soldier in soldiers {
            grid.register(soldier, Domain::Land, true);
        }
        grid.register(plane, Domain::Air, true);
        assert!(!grid.is_on_foot(plane), "what flies is not on foot");
        assert!(grid.is_on_foot(soldiers[0]) && !grid.is_on_foot(tank));

        // Free: room for a vehicle, or for three on foot.
        assert_eq!(grid.room(at, false, |_| true), CELL_ROOM);
        assert_eq!(grid.room(at, true, |_| true), CELL_ROOM);
        assert_eq!(Grid::room_needed(false), CELL_ROOM);
        assert_eq!(Grid::room_needed(true), 1);

        // A vehicle holds it whole.
        assert!(grid.claim(at, tank));
        assert!(grid.claim(at, tank), "claiming again changes nothing");
        assert_eq!(grid.holders(at), [Some(tank), None, None]);
        assert!(!grid.claim(at, truck));
        assert!(!grid.claim(at, soldiers[0]));
        assert!(!grid.has_room(at, true) && !grid.has_room(at, false));
        assert!(grid.room_for(at, tank) && !grid.room_for(at, truck));
        assert!(!grid.room_for(at, soldiers[0]));
        assert_eq!(
            grid.blockers(at, soldiers[0], true),
            [Some(tank), None, None]
        );
        assert_eq!(grid.blockers(at, tank, false), [None; 3]);
        assert_eq!(grid.spot(at, tank), None);
        assert_eq!(grid.standing_point(at, tank), SubPos::centre_of(at));
        // The aircraft above it is in another layer.
        assert!(grid.claim(at, plane));
        assert_eq!(grid.holders_for(at, Domain::Air), [Some(plane), None, None]);
        assert_eq!(grid.occupant_for(at, Domain::Air), Some(plane));
        assert_eq!(grid.standing_point(at, plane), SubPos::centre_of(at));
        grid.release(at, tank);
        assert_eq!(grid.holders(at), [None; 3]);

        // Three on foot, each at the first free spot.
        for (index, soldier) in soldiers[..3].iter().enumerate() {
            assert!(grid.claim(at, *soldier));
            assert_eq!(grid.spot(at, *soldier), Some(index as u8));
            assert_eq!(
                grid.standing_point(at, *soldier),
                SubPos::spot_of(at, index as u8)
            );
            assert_eq!(grid.room(at, true, |_| true), 2 - index as u8);
            assert!(!grid.claim(at, truck), "a vehicle beside infantry");
            assert!(!grid.has_room(at, false));
        }
        assert!(
            grid.claim(at, soldiers[1]),
            "claiming again changes nothing"
        );
        assert_eq!(grid.spot(at, soldiers[1]), Some(1));
        assert!(!grid.claim(at, soldiers[3]), "a fourth");
        assert_eq!(
            grid.holders(at),
            [Some(SimId(10)), Some(SimId(11)), Some(SimId(12))]
        );
        assert_eq!(grid.occupant(at), Some(SimId(10)));
        assert_eq!(
            grid.blockers(at, soldiers[3], true),
            [Some(SimId(10)), Some(SimId(11)), Some(SimId(12))]
        );
        assert_eq!(grid.blockers(at, soldiers[2], true), [None; 3]);
        assert_eq!(
            grid.blockers(at, truck, false),
            [Some(SimId(10)), Some(SimId(11)), Some(SimId(12))]
        );
        // What would be left if only some of them stayed.
        let stays = |id: SimId| id != soldiers[1];
        assert_eq!(grid.room(at, true, stays), 1);
        assert_eq!(grid.room(at, false, stays), 0);
        assert_eq!(grid.room(at, false, |_| false), CELL_ROOM);
        assert!(!grid.blocked(at, soldiers[3], true, stays));
        assert!(grid.blocked(at, soldiers[3], true, |_| true));
        assert!(grid.blocked(at, truck, false, stays));
        assert!(!grid.blocked(at, truck, false, |_| false));

        // A freed spot is taken again, and a preferred spot only while it is free.
        grid.release(at, soldiers[1]);
        assert_eq!(grid.holders(at), [Some(SimId(10)), None, Some(SimId(12))]);
        assert!(grid.claim_spot(at, soldiers[3], Some(2)));
        assert_eq!(grid.spot(at, soldiers[3]), Some(1), "spot 2 is taken");
        let next = CellPos::new(2, 1);
        assert!(grid.claim_spot(next, soldiers[3], Some(2)));
        assert_eq!(grid.spot(next, soldiers[3]), Some(2));
        assert!(grid.claim_spot(next, soldiers[1], Some(9)), "no such spot");
        assert_eq!(grid.spot(next, soldiers[1]), Some(0));

        // A unit is released from the cell it holds and from the neighbour it enters.
        grid.release_around(at, soldiers[3]);
        assert_eq!(grid.spot(at, soldiers[3]), None);
        assert_eq!(grid.spot(next, soldiers[3]), None);
        grid.unregister(soldiers[3]);
        assert!(!grid.is_on_foot(soldiers[3]));

        // Outside the map, and under a building, nothing is claimed.
        assert!(!grid.claim(CellPos::new(4, 0), tank));
        assert_eq!(grid.room(CellPos::new(-1, 0), true, |_| true), 0);
        assert_eq!(grid.holders(CellPos::new(9, 9)), [None; 3]);
        grid.cover(CellPos::new(3, 3), SimId(99));
        assert!(!grid.claim(CellPos::new(3, 3), tank));
        assert!(!grid.claim(CellPos::new(3, 3), soldiers[0]));
        assert!(
            grid.claim(CellPos::new(3, 3), plane),
            "an aircraft flies over it"
        );
    }

    /// RA5: gems are the cells with ore whose look says so; they and the mined ground are part
    /// of the hash, and a grid with neither hashes as it did before RA5 (and before RA3).
    #[test]
    fn gems_and_mined_ground_are_marked_kept_in_step_with_the_ore_and_hashed() {
        let gem_look = crate::map_scale::LOOK_FEATURE_GEMS << 4 | 1;
        let ore_look = crate::map_scale::LOOK_FEATURE_ORE << 4 | 1;
        let mut grid = open(4, 2);
        let with = |ore| Cell {
            ore,
            buildable: false,
            ..*open(1, 1).cell(CellPos::new(0, 0)).expect("a cell")
        };
        for x in 0..3 {
            grid.set_terrain(CellPos::new(x, 0), with(100));
        }
        let plain = hash(&grid);
        assert_eq!(plain, hash_before_ra3(&grid));
        // The looks: gems on cell 0, ore on cell 1, gems on a cell without ore (3), and fewer
        // looks than cells.
        grid.mark_gems(&[gem_look, ore_look, 0, gem_look]);
        assert!(grid.is_gems(CellPos::new(0, 0)));
        assert!(!grid.is_gems(CellPos::new(1, 0)) && !grid.is_gems(CellPos::new(3, 0)));
        assert!(!grid.is_gems(CellPos::new(-1, 0)) && !grid.is_gems(CellPos::new(0, 2)));
        assert_eq!(grid.gem_cells(), 1);
        let marked = hash(&grid);
        assert_ne!(marked, plain);
        // Gems only where ore lies; and back to ore.
        assert!(!grid.set_gems(CellPos::new(3, 0), true));
        assert!(!grid.set_gems(CellPos::new(9, 9), true));
        assert!(grid.set_gems(CellPos::new(1, 0), true));
        assert_ne!(hash(&grid), marked);
        assert!(grid.set_gems(CellPos::new(1, 0), false));
        assert_eq!(hash(&grid), marked);
        // Bails: 100 of gems are 2 (value 50), 100 of ore 4 (value 25), at most 3.
        grid.ore_to_bails(25, 50, 3);
        let bails = |grid: &Grid, x| grid.cell(CellPos::new(x, 0)).expect("a cell").ore;
        assert_eq!([0, 1, 2, 3].map(|x| bails(&grid, x)), [2, 3, 3, 0]);
        // Growth: ore below the most, never gems, never an empty cell.
        assert!(!grid.grow_ore(CellPos::new(0, 0), 3));
        assert!(!grid.grow_ore(CellPos::new(1, 0), 3));
        assert!(grid.grow_ore(CellPos::new(1, 0), 4));
        assert!(!grid.grow_ore(CellPos::new(3, 0), 4));
        assert_eq!([0, 1, 2, 3].map(|x| bails(&grid, x)), [2, 4, 3, 0]);
        // Taking the last bail of gems leaves mined ground that is no gems; taking nothing
        // from an empty cell leaves it as it was.
        assert_eq!(grid.take_ore(CellPos::new(0, 0), 5), 2);
        assert!(!grid.is_gems(CellPos::new(0, 0)));
        assert_eq!(grid.gem_cells(), 0);
        assert!(grid.takes_ore(CellPos::new(0, 0)), "mined ground");
        let mined = hash(&grid);
        assert_eq!(grid.take_ore(CellPos::new(3, 0), 5), 0);
        assert_eq!(hash(&grid), mined);
        assert!(grid.takes_ore(CellPos::new(3, 0)), "building ground");
        assert!(!grid.takes_ore(CellPos::new(1, 0)), "ore lies there");
        // Planted ore is one bail of ore, and the ground is mined ground no longer: with the
        // bail taken off by scenario setup the cell is plain ore ground again.
        assert!(grid.plant_ore(CellPos::new(0, 0)));
        assert!(!grid.plant_ore(CellPos::new(0, 0)));
        assert_eq!(bails(&grid, 0), 1);
        assert_ne!(hash(&grid), mined);
        grid.set_terrain(CellPos::new(0, 0), with(0));
        assert!(!grid.takes_ore(CellPos::new(0, 0)));
        // A building keeps ore off its ground.
        grid.cover(CellPos::new(3, 0), SimId(4));
        assert!(!grid.takes_ore(CellPos::new(3, 0)));
        // The cells of ore in row-major order, without the gems.
        assert!(grid.set_gems(CellPos::new(2, 0), true));
        assert_eq!(grid.ore_cells(), [(CellPos::new(1, 0), 4)]);
    }

    /// RA3: the hash of a grid covers who holds which spot (invariant I3), and the hash of a
    /// grid without units on foot is what it was before RA3 (docs/DECISIONS.md R6).
    #[test]
    fn the_hash_covers_the_spots_and_is_unchanged_without_units_on_foot() {
        let mut plain = open(5, 3);
        plain.register(SimId(1), Domain::Land, false);
        plain.register(SimId(2), Domain::Naval, false);
        plain.register(SimId(3), Domain::Air, false);
        assert!(plain.claim(CellPos::new(0, 0), SimId(1)));
        assert!(plain.claim(CellPos::new(4, 2), SimId(3)));
        plain.cover(CellPos::new(2, 2), SimId(7));
        assert!(plain.spots.is_empty(), "no spots without units on foot");
        assert_eq!(hash(&plain), hash_before_ra3(&plain));
        assert_eq!(hash(&open(5, 3)), hash_before_ra3(&open(5, 3)));

        let at = CellPos::new(1, 1);
        let with = |claims: &[(u32, Option<u8>)]| {
            let mut grid = plain.clone();
            for &(id, spot) in claims {
                grid.register(SimId(id), Domain::Land, true);
                assert!(grid.claim_spot(at, SimId(id), spot));
            }
            hash(&grid)
        };
        let one = with(&[(10, None)]);
        assert_ne!(one, hash(&plain));
        assert_eq!(one, with(&[(10, Some(0))]));
        // Another spot, another unit, another unit beside it, and the two the other way round.
        assert_ne!(one, with(&[(10, Some(1))]));
        assert_ne!(one, with(&[(11, None)]));
        assert_ne!(one, with(&[(10, None), (11, None)]));
        assert_ne!(
            with(&[(10, Some(0)), (11, Some(1))]),
            with(&[(10, Some(1)), (11, Some(0))])
        );
        // A unit on foot that holds no cell yet is part of the state too.
        let mut registered = plain.clone();
        registered.register(SimId(10), Domain::Land, true);
        assert_ne!(hash(&registered), hash(&plain));
    }
}
