//! Maps: the [`Grid`] resource and the loader for the Tiled map format of docs/DECISIONS.md (D8).
//!
//! A map file is validated completely before any of it is used, and every problem is reported as
//! a [`MapError`] naming the file, the offending field, and the reason (invariant I8). The
//! `game` crate hands a map to `bevy_ecs_tiled` only after it has passed [`load_map`].

use std::collections::BTreeMap;
use std::fmt;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use bevy_ecs::resource::Resource;
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use tiled::{Loader, Orientation, Properties, PropertyValue, TileLayer};

use crate::hash::Fnv1a64;

/// The map format version this crate reads (map property `schema_version`).
pub const MAP_SCHEMA_VERSION: i32 = 1;

/// Largest accepted map width and height, in cells.
pub const MAX_MAP_SIDE: u32 = 256;

/// Name of the one tile layer a map consists of.
pub const TERRAIN_LAYER: &str = "terrain";

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

/// Terrain of one cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    /// Ground units may enter the cell.
    pub passable: bool,
    /// Buildings may cover the cell.
    pub buildable: bool,
    /// Ore stored in the cell, in ore units.
    pub ore: u32,
}

/// Per cell terrain of the current map (PLAN.md section 5).
///
/// [`SimPlugin`](crate::SimPlugin) starts with an empty 0×0 grid; match setup replaces it with
/// the grid of a [`LoadedMap`] before the first tick.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct Grid {
    width: u32,
    height: u32,
    /// Row-major: the cell (x, y) is at index `y * width + x`.
    cells: Vec<Cell>,
}

impl Grid {
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

    fn index(&self, pos: CellPos) -> Option<usize> {
        let x = u32::try_from(pos.x).ok()?;
        let y = u32::try_from(pos.y).ok()?;
        if x >= self.width || y >= self.height {
            return None;
        }
        usize::try_from(u64::from(y) * u64::from(self.width) + u64::from(x)).ok()
    }

    /// Width, height, then every cell in row-major order as one flag byte (bit 0 passable,
    /// bit 1 buildable) and the ore amount.
    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        hasher.write_u32(self.width);
        hasher.write_u32(self.height);
        for cell in &self.cells {
            hasher.write(&[u8::from(cell.passable) | (u8::from(cell.buildable) << 1)]);
            hasher.write_u32(cell.ore);
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
        write!(
            f,
            "map {}: {}: {}",
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
        }
    }
}

/// Loads and validates the map file at `path`.
///
/// Never panics on malformed input: every problem is returned as a [`MapError`].
pub fn load_map(path: impl AsRef<Path>) -> Result<LoadedMap, MapError> {
    let path = path.as_ref();
    let error = |field: String, kind: MapErrorKind| MapError {
        file: path.to_owned(),
        field,
        kind,
    };

    let bytes =
        std::fs::read(path).map_err(|e| error("file".into(), MapErrorKind::Read(e.to_string())))?;
    precheck(&bytes).map_err(|(field, kind)| error(field, kind))?;

    // `tiled` reads the map through this closure, so it parses exactly the bytes checked above.
    // Other files (external tilesets) come from the file system.
    let reader = |requested: &Path| -> std::io::Result<Box<dyn Read>> {
        if requested == path {
            Ok(Box::new(Cursor::new(bytes.clone())))
        } else {
            Ok(Box::new(std::io::BufReader::new(std::fs::File::open(
                requested,
            )?)))
        }
    };
    let map = Loader::with_reader(reader)
        .load_tmx_map(path)
        .map_err(|e| error("file".into(), MapErrorKind::Parse(e.to_string())))?;

    validate(&map).map_err(|(field, kind)| error(field, kind))
}

type Issue = (String, MapErrorKind);

/// Checks done on the raw XML before `tiled` parses it.
///
/// The map header is checked here so that a zero sized or oversized map is reported as such
/// rather than as a data error. The layer data check prevents a panic: `tiled` 0.16.0 indexes a
/// finite layer's tile vector without a bounds check (`FiniteTileLayerData::get_tile_data`), so
/// a layer with fewer data entries than `width × height` would panic on access. The entries are
/// counted exactly as `tiled` decodes CSV data: the first text or CDATA node of `<data>`, split
/// on commas.
fn precheck(bytes: &[u8]) -> Result<(), Issue> {
    let mut reader = Reader::from_reader(Cursor::new(bytes));
    let mut buf = Vec::new();
    let mut seen_root = false;
    // Name and expected entry count of the `<layer>` being read.
    let mut layer: Option<(String, u64)> = None;

    loop {
        let event = reader.read_event_into(&mut buf).map_err(xml_issue)?;
        match &event {
            Event::Start(element) | Event::Empty(element) => {
                let is_empty = matches!(event, Event::Empty(_));
                let name = element.local_name();
                if !seen_root {
                    seen_root = true;
                    if name.as_ref() == b"map" {
                        check_map_header(element)?;
                    }
                } else if name.as_ref() == b"layer" && !is_empty {
                    layer = Some(layer_header(element)?);
                } else if name.as_ref() == b"data"
                    && let Some((layer_name, expected)) = &layer
                {
                    let field = format!("layer {layer_name:?} data");
                    let encoding = attribute(element, "encoding")?;
                    let compression = attribute(element, "compression")?;
                    if encoding.as_deref() != Some("csv") || compression.is_some() {
                        return Err((
                            field,
                            MapErrorKind::UnsupportedEncoding {
                                encoding,
                                compression,
                            },
                        ));
                    }
                    let text = if is_empty {
                        String::new()
                    } else {
                        first_text(&mut reader)?
                    };
                    let found = text.split(',').count() as u64;
                    if found != *expected {
                        return Err((
                            field,
                            MapErrorKind::TileCountMismatch {
                                expected: *expected,
                                found,
                            },
                        ));
                    }
                }
            }
            Event::End(element) if element.local_name().as_ref() == b"layer" => layer = None,
            Event::Eof => return Ok(()),
            _ => {}
        }
        buf.clear();
    }
}

fn xml_issue(error: quick_xml::Error) -> Issue {
    (
        "file".into(),
        MapErrorKind::Parse(format!("XML error: {error}")),
    )
}

fn attribute(element: &BytesStart<'_>, name: &str) -> Result<Option<String>, Issue> {
    match element.try_get_attribute(name).map_err(xml_issue)? {
        Some(value) => Ok(Some(
            value.unescape_value().map_err(xml_issue)?.into_owned(),
        )),
        None => Ok(None),
    }
}

fn check_map_header(map: &BytesStart<'_>) -> Result<(), Issue> {
    if let Some(orientation) = attribute(map, "orientation")?
        && orientation != "isometric"
    {
        return Err((
            "map attribute \"orientation\"".into(),
            MapErrorKind::UnsupportedOrientation(orientation),
        ));
    }
    if attribute(map, "infinite")?.as_deref() == Some("1") {
        return Err(("map attribute \"infinite\"".into(), MapErrorKind::Infinite));
    }
    // Missing or malformed sizes are left to `tiled`, which reports them.
    let size = |name| -> Result<Option<u32>, Issue> {
        Ok(attribute(map, name)?.and_then(|value| value.parse().ok()))
    };
    if let (Some(width), Some(height)) = (size("width")?, size("height")?) {
        let field = "map attributes \"width\" and \"height\"".to_string();
        if width == 0 || height == 0 {
            return Err((field, MapErrorKind::ZeroSize { width, height }));
        }
        if width > MAX_MAP_SIDE || height > MAX_MAP_SIDE {
            return Err((field, MapErrorKind::TooLarge { width, height }));
        }
    }
    Ok(())
}

fn layer_header(layer: &BytesStart<'_>) -> Result<(String, u64), Issue> {
    let name = attribute(layer, "name")?.unwrap_or_default();
    let dimension = |attr| -> Result<u64, Issue> {
        attribute(layer, attr)?
            .and_then(|value| value.parse::<u32>().ok())
            .map(u64::from)
            .ok_or_else(|| {
                (
                    format!("layer {name:?} attribute {attr:?}"),
                    MapErrorKind::Parse("missing or not an unsigned integer".into()),
                )
            })
    };
    let expected = dimension("width")? * dimension("height")?;
    Ok((name, expected))
}

/// Reads the rest of a `<data>` element and returns its first text or CDATA node, mirroring
/// `tiled`'s `read_text_or_cdata`.
fn first_text(reader: &mut Reader<Cursor<&[u8]>>) -> Result<String, Issue> {
    let mut buf = Vec::new();
    let mut skip_buf = Vec::new();
    let mut text = None;
    loop {
        match reader.read_event_into(&mut buf).map_err(xml_issue)? {
            Event::Text(t) if text.is_none() => {
                text = Some(t.unescape().map_err(xml_issue)?.into_owned());
            }
            Event::CData(t) if text.is_none() => {
                text = Some(String::from_utf8_lossy(t.as_ref()).into_owned());
            }
            Event::Start(element) => {
                let end = element.to_end().into_owned();
                reader
                    .read_to_end_into(end.name(), &mut skip_buf)
                    .map_err(xml_issue)?;
            }
            Event::End(_) | Event::Eof => return Ok(text.unwrap_or_default()),
            _ => {}
        }
        buf.clear();
    }
}

/// Checks the parsed map against the map format and builds the grid.
fn validate(map: &tiled::Map) -> Result<LoadedMap, Issue> {
    // Repeated here because the header check of `precheck` skips anything `tiled` reports itself.
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

    check_map_properties(&map.properties)?;
    let terrain = tile_terrain(map)?;
    let layer = terrain_layer(map)?;

    let mut cells = Vec::with_capacity(width as usize * height as usize);
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let field = || format!("layer {TERRAIN_LAYER:?} cell ({x}, {y})");
            let tile = layer
                .get_tile(x, y)
                .ok_or_else(|| (field(), MapErrorKind::EmptyCell))?;
            let cell = terrain
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
        }
    }

    Ok(LoadedMap {
        grid: Grid {
            width,
            height,
            cells,
        },
        tile_width: map.tile_width,
        tile_height: map.tile_height,
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

/// Terrain of every tile defined in the map's tilesets, keyed by (tileset index, tile id).
fn tile_terrain(map: &tiled::Map) -> Result<BTreeMap<(usize, u32), Cell>, Issue> {
    let mut terrain = BTreeMap::new();
    for (index, tileset) in map.tilesets().iter().enumerate() {
        let tileset_field = format!("tileset {:?}", tileset.name);
        if let Some(name) = sorted_names(&tileset.properties).first() {
            return Err((
                format!("{tileset_field} property {name:?}"),
                MapErrorKind::UnknownProperty,
            ));
        }
        if tileset.image.is_none() {
            return Err((tileset_field, MapErrorKind::TilesetWithoutImage));
        }
        let mut tiles: Vec<_> = tileset.tiles().collect();
        tiles.sort_by_key(|(id, _)| *id);
        for (id, tile) in tiles {
            let cell = parse_tile(&format!("{tileset_field} tile {id}"), &tile.properties)?;
            terrain.insert((index, id), cell);
        }
    }
    Ok(terrain)
}

fn parse_tile(tile_field: &str, properties: &Properties) -> Result<Cell, Issue> {
    let field = |name: &str| format!("{tile_field} property {name:?}");
    if let Some(unknown) = sorted_names(properties)
        .into_iter()
        .find(|name| !matches!(name.as_str(), "passable" | "buildable" | "ore"))
    {
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

    let passable = bool_property("passable")?;
    let buildable = bool_property("buildable")?;
    let ore = match get("ore")? {
        PropertyValue::IntValue(value) => {
            u32::try_from(*value).map_err(|_| (field("ore"), MapErrorKind::NegativeOre(*value)))?
        }
        other => return Err(wrong_type("ore", "int", other)),
    };
    Ok(Cell {
        passable,
        buildable,
        ore,
    })
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
