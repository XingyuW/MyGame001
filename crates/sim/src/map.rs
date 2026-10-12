//! Maps: the [`Grid`] resource and the loader for the Tiled map format of docs/DECISIONS.md (D8,
//! D13).
//!
//! A map file is validated completely before any of it is used, and every problem is reported as
//! a [`MapError`] naming the file, the offending field, and the reason (invariant I8). Before
//! `tiled` parses anything, the raw file is checked against the supported subset of the format
//! (`map_subset`, invariant I11). The `game` crate hands a map to `bevy_ecs_tiled` only after it
//! has passed [`load_map`] and the game's own check of the tileset image.

use std::collections::BTreeMap;
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

    /// The cell that contains this position.
    pub const fn cell(self) -> CellPos {
        CellPos {
            x: self.x.div_euclid(SUBCELLS_PER_CELL),
            y: self.y.div_euclid(SUBCELLS_PER_CELL),
        }
    }
}

/// Terrain of one cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    /// Ground units may enter the cell.
    pub passable: bool,
    /// Explicit water terrain; rocks are never navigable by ships.
    pub water: bool,
    /// Buildings may cover the cell.
    pub buildable: bool,
    /// Ore stored in the cell, in ore units.
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
    /// Row-major, as `cells`: the ground or naval unit holding the cell.
    occupants: Vec<Option<SimId>>,
    /// Row-major, as `cells`: the aircraft holding the cell.
    air_occupants: Vec<Option<SimId>>,
    /// Row-major, as `cells`: the building covering the cell.
    buildings: Vec<Option<SimId>>,
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
            air_occupants: vec![None; count],
            buildings: vec![None; count],
            cells,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
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
    pub(crate) fn register_domain(&mut self, id: SimId, domain: crate::Domain) {
        if domain != crate::Domain::Land {
            self.domains.insert(id, domain);
        }
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
    /// equation 4).
    pub(crate) fn take_ore(&mut self, pos: CellPos, max: u32) -> u32 {
        match self.index(pos).and_then(|index| self.cells.get_mut(index)) {
            Some(cell) => {
                let taken = cell.ore.min(max);
                cell.ore -= taken;
                taken
            }
            None => 0,
        }
    }

    /// The unit standing in or entering `pos`, if any, in the occupancy layer of this view's
    /// navigation domain: aircraft for an air view, ground and naval units otherwise.
    pub fn occupant(&self, pos: CellPos) -> Option<SimId> {
        self.occupant_for(pos, self.navigation)
    }

    /// The unit holding `pos` in the occupancy layer that units of `domain` move in.
    pub fn occupant_for(&self, pos: CellPos, domain: crate::Domain) -> Option<SimId> {
        self.index(pos)
            .and_then(|index| self.layer(domain).get(index))
            .copied()
            .flatten()
    }

    fn layer(&self, domain: crate::Domain) -> &Vec<Option<SimId>> {
        if domain == crate::Domain::Air {
            &self.air_occupants
        } else {
            &self.occupants
        }
    }

    /// Replaces the terrain of `pos` and returns whether `pos` is inside the map.
    ///
    /// Scenario setup only (tests and match setup), like replacing the whole grid; during a match
    /// terrain does not change, and cells become impassable only by being covered by a building
    /// placed with a command (M3). A unit standing on a cell that becomes impassable may still
    /// leave it, and units whose path crosses it repath.
    pub fn set_terrain(&mut self, pos: CellPos, cell: Cell) -> bool {
        match self.index(pos).and_then(|index| self.cells.get_mut(index)) {
            Some(slot) => {
                *slot = cell;
                true
            }
            None => false,
        }
    }

    /// Makes `unit` the occupant of `pos` if the cell is inside the map, passable, and free (or
    /// already held by `unit`). Returns whether `unit` now holds the cell.
    pub(crate) fn claim(&mut self, pos: CellPos, unit: SimId) -> bool {
        let domain = self.domain_of(unit);
        if !self.passable_for(pos, domain) {
            return false;
        }
        let Some(index) = self.index(pos) else {
            return false;
        };
        let layer = if domain == crate::Domain::Air {
            &mut self.air_occupants
        } else {
            &mut self.occupants
        };
        match layer.get_mut(index) {
            Some(slot @ None) => {
                *slot = Some(unit);
                true
            }
            Some(Some(holder)) => *holder == unit,
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
    /// terrain, bit 1 buildable, bit 2 occupied, bit 3 covered by a building), the ore amount,
    /// and the occupant's and the building's ids where set (docs/DECISIONS.md D11, extended in
    /// M2 and M3).
    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        hasher.write_u64(self.domains.len() as u64);
        for (id, domain) in &self.domains {
            hasher.write_u32(id.0);
            hasher.write(&[*domain as u8]);
        }
        hasher.write_u32(self.width);
        hasher.write_u32(self.height);
        for ((cell, occupant), building) in
            self.cells.iter().zip(&self.occupants).zip(&self.buildings)
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
        for (index, occupant) in self.air_occupants.iter().enumerate() {
            if let Some(id) = occupant {
                hasher.write_u64(index as u64);
                hasher.write_u32(id.0);
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
    /// tile has none). Presentation data, not used by the simulation: it tells the renderer what
    /// stands on a cell (a forest, a city block, a bridge) whose terrain is the same to the
    /// simulation. The game defines and checks the values.
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
    let air_occupants = vec![None; cells.len()];
    let buildings = vec![None; cells.len()];
    Ok(LoadedMap {
        grid: Grid {
            navigation: crate::Domain::Land,
            domains: BTreeMap::new(),
            width: map.width,
            height: map.height,
            cells,
            occupants,
            air_occupants,
            buildings,
        },
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
