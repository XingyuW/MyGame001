//! The supported subset of the Tiled map format (docs/DECISIONS.md, D8 and D13; invariant I11).
//!
//! `tiled` and `bevy_ecs_tiled` accept far more of the Tiled format than this game uses, and some
//! of what they accept makes them panic, recurse without bound, or allocate without bound (M1
//! review 1, P0-1 to P0-4). Every map file and external tileset is therefore checked against an
//! allow-list on its raw bytes before either of them sees it:
//!
//! * each element must be one listed below, inside its listed parent, and at most as often as
//!   listed;
//! * each attribute must be one listed for its element, appear at most once, and carry no
//!   namespace prefix (names are compared in full, while `tiled` matches attributes and elements
//!   by their local name, so `a:width` would reach it as `width`);
//! * every value that `tiled` or `bevy_ecs_tiled` computes with is restricted to the range named
//!   below, and every property value parses as its declared type exactly as `tiled` parses it;
//! * only whitespace and comments may appear between elements, and nothing but plain CSV text
//!   inside `<data>`; DTDs, processing instructions, and CDATA sections are rejected;
//! * referenced files are plain file names in the map's directory, and the external file
//!   references that `bevy_ecs_tiled` finds with its own line-based scan are exactly the
//!   references accepted here.
//!
//! The XML is read with the `quick-xml` version and reader settings that `tiled` uses. The checker
//! keeps no stack of its own: the grammar is at most five elements deep, and it returns at the
//! first element outside the grammar, so its recursion is bounded however deeply a file nests.
//! Anything outside the subset is a typed [`MapError`].

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};

use quick_xml::Reader;
use quick_xml::events::{BytesStart, BytesText, Event};

use crate::map::{
    MAX_MAP_FILE_BYTES, MAX_MAP_SIDE, MAX_TILESET_IMAGE_SIDE_PX, MAX_TILESET_TILES, MapError,
    MapErrorKind, TERRAIN_LAYER, TILE_HEIGHT_PX, TILE_WIDTH_PX,
};

/// An external tileset that passed the check: the path at which `tiled` will request it (the
/// map's directory joined with the `source` attribute, as `tiled` resolves it) and the bytes
/// that were checked, which are the bytes `tiled` must parse.
pub(crate) struct CheckedTileset {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}

/// Reads a map or tileset file without reading more than [`MAX_MAP_FILE_BYTES`] + 1 bytes.
pub(crate) fn read_file(path: &Path) -> Result<Vec<u8>, MapErrorKind> {
    let file = std::fs::File::open(path).map_err(|e| MapErrorKind::Read(e.to_string()))?;
    let mut bytes = Vec::new();
    file.take(MAX_MAP_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| MapErrorKind::Read(e.to_string()))?;
    if u64::try_from(bytes.len()).map_or(true, |len| len > MAX_MAP_FILE_BYTES) {
        return Err(MapErrorKind::FileTooLarge {
            max_bytes: MAX_MAP_FILE_BYTES,
        });
    }
    Ok(bytes)
}

/// Checks a map file (`bytes`, read from `path`) and the external tileset it references, if any.
/// Returns the checked external tileset.
pub(crate) fn check_map(path: &Path, bytes: &[u8]) -> Result<Option<CheckedTileset>, MapError> {
    let text = utf8(path, bytes)?;
    let mut checker = Checker::new(path, bytes);
    let (root, empty) = checker.root("map")?;
    let (tileset, source) = checker.map(&root, empty)?;
    checker.end_of_document()?;
    check_external_references(path, text, source.as_slice())?;
    Ok(tileset)
}

/// Checks an external tileset file. Every error names the tileset file.
fn check_tileset_file(path: &Path, bytes: &[u8]) -> Result<(), MapError> {
    let text = utf8(path, bytes)?;
    let mut checker = Checker::new(path, bytes);
    let (root, empty) = checker.root("tileset")?;
    let field = format!("tileset {:?}", label(&root, "name"));
    let attributes = checker.attributes(&root, &field, &TSX_TILESET_ATTRIBUTES)?;
    checker.tileset_body(&attributes, &field, empty)?;
    checker.end_of_document()?;
    check_external_references(path, text, &[])
}

const MAP_ATTRIBUTES: [&str; 11] = [
    "version",
    "tiledversion",
    "orientation",
    "renderorder",
    "width",
    "height",
    "tilewidth",
    "tileheight",
    "infinite",
    "nextlayerid",
    "nextobjectid",
];
const EMBEDDED_TILESET_ATTRIBUTES: [&str; 6] = [
    "firstgid",
    "name",
    "tilewidth",
    "tileheight",
    "tilecount",
    "columns",
];
const TSX_TILESET_ATTRIBUTES: [&str; 7] = [
    "version",
    "tiledversion",
    "name",
    "tilewidth",
    "tileheight",
    "tilecount",
    "columns",
];
const UNSIGNED: &str = "an unsigned 32-bit integer";

/// The attributes of one element, in file order, unescaped as `tiled` unescapes them.
struct Attributes(Vec<(String, String)>);

impl Attributes {
    fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

struct Checker<'a> {
    file: &'a Path,
    bytes: &'a [u8],
    reader: Reader<&'a [u8]>,
}

impl<'a> Checker<'a> {
    fn new(file: &'a Path, bytes: &'a [u8]) -> Self {
        Self {
            file,
            bytes,
            reader: Reader::from_reader(bytes),
        }
    }

    fn error(&self, field: impl Into<String>, kind: MapErrorKind) -> MapError {
        MapError {
            file: self.file.to_owned(),
            field: field.into(),
            kind,
        }
    }

    fn xml_error(&self, field: &str, error: impl std::fmt::Display) -> MapError {
        self.error(field, MapErrorKind::Parse(format!("XML error: {error}")))
    }

    fn invalid(&self, field: impl Into<String>, found: impl ToString, expected: &str) -> MapError {
        self.error(
            field,
            MapErrorKind::InvalidValue {
                found: found.to_string(),
                expected: expected.to_owned(),
            },
        )
    }

    fn event(&mut self, field: &str) -> Result<Event<'a>, MapError> {
        let event = self.reader.read_event();
        event.map_err(|e| self.xml_error(field, e))
    }

    /// Reads up to the root element, which must be `<name>`. Only an XML declaration (first),
    /// comments, and whitespace may precede it.
    fn root(&mut self, name: &str) -> Result<(BytesStart<'a>, bool), MapError> {
        let mut first = true;
        loop {
            let event = self.event("file")?;
            let (element, empty) = match event {
                Event::Decl(_) if std::mem::replace(&mut first, false) => continue,
                Event::Comment(_) => {
                    first = false;
                    continue;
                }
                Event::Text(text) if is_whitespace(&text) => {
                    first = false;
                    continue;
                }
                Event::Start(element) => (element, false),
                Event::Empty(element) => (element, true),
                Event::Eof => {
                    return Err(self.error(
                        "file",
                        MapErrorKind::Parse(format!("the file has no root element <{name}>")),
                    ));
                }
                other => {
                    return Err(self.error(
                        "file",
                        MapErrorKind::Parse(format!(
                            "{} before the root element is not supported",
                            describe(&other)
                        )),
                    ));
                }
            };
            if element.name().as_ref() != name.as_bytes() {
                return Err(self.error(
                    "file",
                    MapErrorKind::Parse(format!(
                        "expected the root element <{name}>, found <{}>",
                        element_name(&element)
                    )),
                ));
            }
            return Ok((element, empty));
        }
    }

    /// After the root element: its end tag must be complete (`quick-xml` accepts `</map` cut off
    /// at the end of the file), and only comments and whitespace may follow.
    fn end_of_document(&mut self) -> Result<(), MapError> {
        let end = self.reader.buffer_position();
        if end == 0 || self.bytes.get(end - 1) != Some(&b'>') {
            return Err(self.error(
                "file",
                MapErrorKind::Parse("the root element's end tag is not terminated".into()),
            ));
        }
        loop {
            match self.event("file")? {
                Event::Comment(_) => {}
                Event::Text(text) if is_whitespace(&text) => {}
                Event::Eof => return Ok(()),
                other => {
                    return Err(self.error(
                        "file",
                        MapErrorKind::Parse(format!(
                            "{} after the root element is not supported",
                            describe(&other)
                        )),
                    ));
                }
            }
        }
    }

    /// The next child element of the element that `field` names, or `None` at its end tag.
    /// Comments and whitespace are skipped; any other content is rejected.
    fn child(&mut self, field: &str) -> Result<Option<(BytesStart<'a>, bool)>, MapError> {
        loop {
            match self.event(field)? {
                Event::Start(element) => return Ok(Some((element, false))),
                Event::Empty(element) => return Ok(Some((element, true))),
                Event::End(_) => return Ok(None),
                Event::Comment(_) => {}
                Event::Text(text) if is_whitespace(&text) => {}
                Event::Eof => {
                    return Err(self.error(
                        field,
                        MapErrorKind::Parse("the file ends inside this element".into()),
                    ));
                }
                other => {
                    return Err(self.error(
                        field,
                        MapErrorKind::Parse(format!("{} is not supported here", describe(&other))),
                    ));
                }
            }
        }
    }

    /// [`Self::child`] for an element that may be empty (`<name/>`), which has no children.
    fn child_unless_empty(
        &mut self,
        field: &str,
        empty: bool,
    ) -> Result<Option<(BytesStart<'a>, bool)>, MapError> {
        if empty { Ok(None) } else { self.child(field) }
    }

    /// Rejects any child element of a non-empty element that may have none.
    fn no_children(&mut self, field: &str) -> Result<(), MapError> {
        match self.child(field)? {
            Some((child, _)) => Err(self.unsupported_element(field, &child)),
            None => Ok(()),
        }
    }

    fn unsupported_element(&self, owner: &str, element: &BytesStart<'_>) -> MapError {
        self.error(
            format!("{owner} element <{}>", element_name(element)),
            MapErrorKind::UnsupportedElement,
        )
    }

    /// Fails if `seen` is already set (an element that may appear at most once), then sets it.
    fn once(&self, seen: &mut bool, owner: &str, element: &BytesStart<'_>) -> Result<(), MapError> {
        if std::mem::replace(seen, true) {
            return Err(self.error(
                format!("{owner} element <{}>", element_name(element)),
                MapErrorKind::Duplicate,
            ));
        }
        Ok(())
    }

    /// The element's attributes; each must be in `allowed` (compared by full name, so a
    /// namespaced name never matches) and appear once.
    fn attributes(
        &self,
        element: &BytesStart<'_>,
        field: &str,
        allowed: &[&str],
    ) -> Result<Attributes, MapError> {
        let mut values: Vec<(String, String)> = Vec::new();
        for attribute in element.attributes().with_checks(false) {
            let attribute = attribute.map_err(|e| self.xml_error(field, e))?;
            let key = String::from_utf8_lossy(attribute.key.as_ref()).into_owned();
            let attribute_field = format!("{field} attribute {key:?}");
            if !allowed.contains(&key.as_str()) {
                return Err(self.error(attribute_field, MapErrorKind::UnsupportedAttribute));
            }
            if values.iter().any(|(seen, _)| *seen == key) {
                return Err(self.error(attribute_field, MapErrorKind::Duplicate));
            }
            let value = attribute
                .unescape_value()
                .map_err(|e| self.xml_error(&attribute_field, e))?
                .into_owned();
            values.push((key, value));
        }
        Ok(Attributes(values))
    }

    fn required<'v>(
        &self,
        attributes: &'v Attributes,
        owner: &str,
        name: &str,
    ) -> Result<&'v str, MapError> {
        attributes.get(name).ok_or_else(|| {
            self.error(
                format!("{owner} attribute {name:?}"),
                MapErrorKind::MissingAttribute,
            )
        })
    }

    fn unsigned(&self, attributes: &Attributes, owner: &str, name: &str) -> Result<u32, MapError> {
        let value = self.required(attributes, owner, name)?;
        value
            .parse()
            .map_err(|_| self.invalid(format!("{owner} attribute {name:?}"), value, UNSIGNED))
    }

    fn map(
        &mut self,
        element: &BytesStart<'a>,
        empty: bool,
    ) -> Result<(Option<CheckedTileset>, Vec<String>), MapError> {
        let attributes = self.attributes(element, "map", &MAP_ATTRIBUTES)?;
        self.map_header(&attributes)?;
        let mut tileset = None;
        let mut source = Vec::new();
        let (mut properties, mut tileset_seen, mut layer_seen) = (false, false, false);
        while let Some((child, child_empty)) = self.child_unless_empty("map", empty)? {
            match child.name().as_ref() {
                b"properties" => {
                    self.once(&mut properties, "map", &child)?;
                    self.properties(&child, child_empty, "map")?;
                }
                b"tileset" => {
                    self.once(&mut tileset_seen, "map", &child)?;
                    // `tiled` resolves the layer's tile ids against the tilesets read before it,
                    // so a tileset after the layer would leave every cell without a tile (M1
                    // review 2, P2-A item 2).
                    if layer_seen {
                        return Err(self.error(
                            "map element <tileset>",
                            MapErrorKind::OutOfOrder {
                                before: format!("the layer {TERRAIN_LAYER:?}"),
                            },
                        ));
                    }
                    if let Some((checked, name)) = self.tileset_in_map(&child, child_empty)? {
                        tileset = Some(checked);
                        source.push(name);
                    }
                }
                b"layer" => {
                    let name = label(&child, "name");
                    if name != TERRAIN_LAYER || std::mem::replace(&mut layer_seen, true) {
                        return Err(
                            self.error(format!("layer {name:?}"), MapErrorKind::UnexpectedLayer)
                        );
                    }
                    self.layer(&child, child_empty, &name)?;
                }
                b"objectgroup" | b"imagelayer" | b"group" => {
                    return Err(self.error(
                        format!("layer {:?}", label(&child, "name")),
                        MapErrorKind::UnexpectedLayer,
                    ));
                }
                _ => return Err(self.unsupported_element("map", &child)),
            }
        }
        // Exactly one tileset (D13); without it every cell would be reported as empty.
        if !tileset_seen {
            return Err(self.error("map element <tileset>", MapErrorKind::MissingElement));
        }
        Ok((tileset, source))
    }

    fn map_header(&self, attributes: &Attributes) -> Result<(), MapError> {
        match attributes.get("orientation") {
            Some("isometric") => {}
            Some(other) => {
                return Err(self.error(
                    "map attribute \"orientation\"",
                    MapErrorKind::UnsupportedOrientation(other.to_owned()),
                ));
            }
            None => {
                return Err(self.error(
                    "map attribute \"orientation\"",
                    MapErrorKind::MissingAttribute,
                ));
            }
        }
        match attributes.get("infinite") {
            None | Some("0") => {}
            Some("1") => {
                return Err(self.error("map attribute \"infinite\"", MapErrorKind::Infinite));
            }
            Some(other) => return Err(self.invalid("map attribute \"infinite\"", other, "\"0\"")),
        }

        let width = self.unsigned(attributes, "map", "width")?;
        let height = self.unsigned(attributes, "map", "height")?;
        let size_field = "map attributes \"width\" and \"height\"";
        if width == 0 || height == 0 {
            return Err(self.error(size_field, MapErrorKind::ZeroSize { width, height }));
        }
        if width > MAX_MAP_SIDE || height > MAX_MAP_SIDE {
            return Err(self.error(size_field, MapErrorKind::TooLarge { width, height }));
        }

        let tile_width = self.unsigned(attributes, "map", "tilewidth")?;
        let tile_height = self.unsigned(attributes, "map", "tileheight")?;
        let tile_field = "map attributes \"tilewidth\" and \"tileheight\"";
        if tile_width == 0 || tile_height == 0 {
            return Err(self.error(
                tile_field,
                MapErrorKind::ZeroTileSize {
                    width: tile_width,
                    height: tile_height,
                },
            ));
        }
        if (tile_width, tile_height) != (TILE_WIDTH_PX, TILE_HEIGHT_PX) {
            return Err(self.invalid(
                tile_field,
                format!("{tile_width}x{tile_height}"),
                &format!(
                    "{TILE_WIDTH_PX}x{TILE_HEIGHT_PX}, the only tile size for which exact picking \
                     is established"
                ),
            ));
        }

        self.required(attributes, "map", "version")?;
        if let Some(order) = attributes.get("renderorder")
            && order != "right-down"
        {
            return Err(self.invalid("map attribute \"renderorder\"", order, "\"right-down\""));
        }
        for name in ["nextlayerid", "nextobjectid"] {
            if attributes.get(name).is_some() {
                self.unsigned(attributes, "map", name)?;
            }
        }
        Ok(())
    }

    /// `<properties>` of the element that `owner` names: only `<property>` children with unique
    /// names, each with a `name`, a `value`, and an optional scalar `type`.
    fn properties(
        &mut self,
        element: &BytesStart<'a>,
        empty: bool,
        owner: &str,
    ) -> Result<(), MapError> {
        let field = format!("{owner} element <properties>");
        self.attributes(element, &field, &[])?;
        if empty {
            return Ok(());
        }
        let mut names = BTreeSet::new();
        while let Some((child, child_empty)) = self.child(&field)? {
            if child.name().as_ref() != b"property" {
                return Err(self.unsupported_element(&field, &child));
            }
            let property = format!("{owner} property {:?}", label(&child, "name"));
            let attributes = self.attributes(&child, &property, &["name", "type", "value"])?;
            let name = self.required(&attributes, &property, "name")?;
            if !names.insert(name.to_owned()) {
                return Err(self.error(property, MapErrorKind::Duplicate));
            }
            // Only scalar types: `class` and `list` values nest properties, which `tiled` parses
            // recursively (M1 review 1, P0-2).
            let kind = attributes.get("type").unwrap_or("string");
            if !matches!(kind, "string" | "int" | "float" | "bool") {
                return Err(self.invalid(
                    format!("{property} attribute \"type\""),
                    kind,
                    "\"string\", \"int\", \"float\", or \"bool\"",
                ));
            }
            // `tiled` reads a property without `value` from the element's text; not supported.
            let value = self.required(&attributes, &property, "value")?;
            let parses = match kind {
                "int" => value.parse::<i32>().is_ok(),
                "float" => value.parse::<f32>().is_ok(),
                "bool" => value.parse::<bool>().is_ok(),
                _ => true,
            };
            if !parses {
                return Err(self.invalid(
                    format!("{property} attribute \"value\""),
                    value,
                    &format!("a value of type {kind}"),
                ));
            }
            if !child_empty {
                self.no_children(&property)?;
            }
        }
        Ok(())
    }

    /// A `<tileset>` inside the map: either embedded, or a reference to an external tileset file
    /// in the map's directory. Returns the checked external tileset and its `source`.
    fn tileset_in_map(
        &mut self,
        element: &BytesStart<'a>,
        empty: bool,
    ) -> Result<Option<(CheckedTileset, String)>, MapError> {
        let is_reference = element.try_get_attribute("source").ok().flatten().is_some();
        if !is_reference {
            let field = format!("tileset {:?}", label(element, "name"));
            let attributes = self.attributes(element, &field, &EMBEDDED_TILESET_ATTRIBUTES)?;
            self.first_gid(&attributes, &field)?;
            self.tileset_body(&attributes, &field, empty)?;
            return Ok(None);
        }

        let field = format!("tileset {:?}", label(element, "source"));
        let attributes = self.attributes(element, &field, &["firstgid", "source"])?;
        self.first_gid(&attributes, &field)?;
        let source = self.required(&attributes, &field, "source")?;
        if !is_plain_file_name(source, ".tsx") {
            return Err(self.invalid(
                format!("{field} attribute \"source\""),
                source,
                &plain_file_name_rule(".tsx"),
            ));
        }
        if !empty {
            // `tiled` would read the content of a non-empty reference as content of the map.
            return Err(self.invalid(
                field,
                "a non-empty <tileset> element",
                "an empty element <tileset firstgid=\"1\" source=\"name.tsx\"/>",
            ));
        }
        // Resolved exactly as `tiled` resolves it (`Tileset::parse_xml_reference`).
        let directory = self.file.parent().ok_or_else(|| {
            self.error(
                "file",
                MapErrorKind::Parse("the map path has no parent directory".into()),
            )
        })?;
        let path = directory.join(source);
        // A file that cannot be read is a fault of the reference, named by its `source`
        // attribute (M1 review 2, P2-A item 3); a file that is too large is a fault of the file.
        let bytes = read_file(&path).map_err(|kind| match kind {
            MapErrorKind::Read(reason) => self.error(
                format!("{field} attribute \"source\""),
                MapErrorKind::Read(format!("{}: {reason}", path.display())),
            ),
            kind => MapError {
                file: path.clone(),
                field: "file".into(),
                kind,
            },
        })?;
        check_tileset_file(&path, &bytes)?;
        Ok(Some((CheckedTileset { path, bytes }, source.to_owned())))
    }

    fn first_gid(&self, attributes: &Attributes, owner: &str) -> Result<(), MapError> {
        let value = self.required(attributes, owner, "firstgid")?;
        if value != "1" {
            return Err(self.invalid(format!("{owner} attribute \"firstgid\""), value, "1"));
        }
        Ok(())
    }

    /// The attributes and content shared by embedded and external tilesets: a single image of
    /// `columns × rows` tiles of the map's tile size, without margin or spacing, `tilecount`
    /// equal to `columns × rows` and at most [`MAX_TILESET_TILES`], and tile definitions with
    /// unique ids below `tilecount` that carry only properties.
    fn tileset_body(
        &mut self,
        attributes: &Attributes,
        field: &str,
        empty: bool,
    ) -> Result<(), MapError> {
        let name = self.required(attributes, field, "name")?;
        if !is_plain_name(name) {
            return Err(self.invalid(
                format!("{field} attribute \"name\""),
                name,
                "a name of ASCII letters, digits, '_', '-', and '.'",
            ));
        }
        let tile_width = self.unsigned(attributes, field, "tilewidth")?;
        let tile_height = self.unsigned(attributes, field, "tileheight")?;
        if (tile_width, tile_height) != (TILE_WIDTH_PX, TILE_HEIGHT_PX) {
            return Err(self.invalid(
                format!("{field} attributes \"tilewidth\" and \"tileheight\""),
                format!("{tile_width}x{tile_height}"),
                &format!("{TILE_WIDTH_PX}x{TILE_HEIGHT_PX}, the map's tile size"),
            ));
        }
        let tilecount = self.unsigned(attributes, field, "tilecount")?;
        if tilecount == 0 || tilecount > MAX_TILESET_TILES {
            return Err(self.invalid(
                format!("{field} attribute \"tilecount\""),
                tilecount,
                &format!("1 to {MAX_TILESET_TILES}"),
            ));
        }
        let columns = self.unsigned(attributes, field, "columns")?;
        if columns == 0 {
            return Err(self.invalid(
                format!("{field} attribute \"columns\""),
                columns,
                "at least 1",
            ));
        }

        let mut image = None;
        if !empty {
            let mut properties = false;
            let mut ids = BTreeSet::new();
            while let Some((child, child_empty)) = self.child(field)? {
                match child.name().as_ref() {
                    b"properties" => {
                        self.once(&mut properties, field, &child)?;
                        self.properties(&child, child_empty, field)?;
                    }
                    b"image" => {
                        if image.is_some() {
                            return Err(self.error(
                                format!("{field} element <image>"),
                                MapErrorKind::Duplicate,
                            ));
                        }
                        image = Some(self.image(&child, child_empty, field)?);
                    }
                    b"tile" => self.tile(&child, child_empty, field, tilecount, &mut ids)?,
                    _ => return Err(self.unsupported_element(field, &child)),
                }
            }
        }

        let Some((image_width, image_height)) = image else {
            return Err(self.error(field, MapErrorKind::TilesetWithoutImage));
        };
        // The renderer uploads the image as one texture (M1 review 2, P1-A).
        for (name, value) in [("width", image_width), ("height", image_height)] {
            if value > MAX_TILESET_IMAGE_SIDE_PX {
                return Err(self.invalid(
                    format!("{field} image attribute {name:?}"),
                    value,
                    &format!(
                        "at most {MAX_TILESET_IMAGE_SIDE_PX} px, the largest texture side every \
                         GPU supports"
                    ),
                ));
            }
        }
        let expected_width = u64::from(columns) * u64::from(tile_width);
        if u64::from(image_width) != expected_width {
            return Err(self.invalid(
                format!("{field} image attribute \"width\""),
                image_width,
                &format!("{columns} columns x {tile_width} px = {expected_width}"),
            ));
        }
        if image_height % tile_height != 0 {
            return Err(self.invalid(
                format!("{field} image attribute \"height\""),
                image_height,
                &format!("a multiple of the tile height {tile_height}"),
            ));
        }
        let rows = image_height / tile_height;
        let expected_count = u64::from(columns) * u64::from(rows);
        if u64::from(tilecount) != expected_count {
            return Err(self.invalid(
                format!("{field} attribute \"tilecount\""),
                tilecount,
                &format!("{columns} columns x {rows} rows of the image = {expected_count}"),
            ));
        }
        Ok(())
    }

    /// `<image>` of a tileset: a PNG file in the map's directory and its size in pixels.
    fn image(
        &mut self,
        element: &BytesStart<'a>,
        empty: bool,
        owner: &str,
    ) -> Result<(u32, u32), MapError> {
        let field = format!("{owner} image");
        let attributes = self.attributes(element, &field, &["source", "width", "height"])?;
        let source = self.required(&attributes, &field, "source")?;
        if !is_plain_file_name(source, ".png") {
            return Err(self.invalid(
                format!("{field} attribute \"source\""),
                source,
                &plain_file_name_rule(".png"),
            ));
        }
        let mut size = [0; 2];
        for (slot, name) in size.iter_mut().zip(["width", "height"]) {
            // `tiled` parses the image size as `i32`.
            let value = self.required(&attributes, &field, name)?;
            *slot = value
                .parse::<i32>()
                .ok()
                .and_then(|value| u32::try_from(value).ok())
                .filter(|&value| value > 0)
                .ok_or_else(|| {
                    self.invalid(
                        format!("{field} attribute {name:?}"),
                        value,
                        "a positive 32-bit integer",
                    )
                })?;
        }
        if !empty {
            self.no_children(&field)?;
        }
        Ok((size[0], size[1]))
    }

    /// `<tile>` of a tileset: a unique `id` below `tilecount` and at most one `<properties>`.
    /// Animations, collision shapes, and per-tile images are not supported.
    fn tile(
        &mut self,
        element: &BytesStart<'a>,
        empty: bool,
        owner: &str,
        tilecount: u32,
        ids: &mut BTreeSet<u32>,
    ) -> Result<(), MapError> {
        let field = format!("{owner} tile {}", label(element, "id"));
        let attributes = self.attributes(element, &field, &["id"])?;
        let id = self.unsigned(&attributes, &field, "id")?;
        if id >= tilecount {
            return Err(self.invalid(
                format!("{field} attribute \"id\""),
                id,
                &format!("a tile id below the tileset's tilecount of {tilecount}"),
            ));
        }
        if !ids.insert(id) {
            return Err(self.error(field, MapErrorKind::Duplicate));
        }
        if empty {
            return Ok(());
        }
        let mut properties = false;
        while let Some((child, child_empty)) = self.child(&field)? {
            if child.name().as_ref() != b"properties" {
                return Err(self.unsupported_element(&field, &child));
            }
            self.once(&mut properties, &field, &child)?;
            self.properties(&child, child_empty, &field)?;
        }
        Ok(())
    }

    /// The terrain `<layer>`: its size, at most one `<properties>`, and at most one `<data>`.
    fn layer(&mut self, element: &BytesStart<'a>, empty: bool, name: &str) -> Result<(), MapError> {
        let field = format!("layer {name:?}");
        let attributes = self.attributes(element, &field, &["id", "name", "width", "height"])?;
        if attributes.get("id").is_some() {
            self.unsigned(&attributes, &field, "id")?;
        }
        let width = self.unsigned(&attributes, &field, "width")?;
        let height = self.unsigned(&attributes, &field, "height")?;
        if empty {
            return Ok(());
        }
        let (mut properties, mut data) = (false, false);
        while let Some((child, child_empty)) = self.child(&field)? {
            match child.name().as_ref() {
                b"properties" => {
                    self.once(&mut properties, &field, &child)?;
                    self.properties(&child, child_empty, &field)?;
                }
                b"data" => {
                    self.once(&mut data, &field, &child)?;
                    let expected = u64::from(width) * u64::from(height);
                    self.data(&child, child_empty, &field, expected)?;
                }
                _ => return Err(self.unsupported_element(&field, &child)),
            }
        }
        Ok(())
    }

    /// Layer `<data>`: uncompressed CSV with exactly `expected` entries, as one plain text node.
    ///
    /// The check prevents a panic: `tiled` 0.16.0 indexes a finite layer's tile vector without a
    /// bounds check (`FiniteTileLayerData::get_tile_data`), so a layer with fewer entries than
    /// `width × height` would panic on access. Entries are counted as `tiled` decodes CSV: the
    /// unescaped text, split on commas.
    fn data(
        &mut self,
        element: &BytesStart<'a>,
        empty: bool,
        layer: &str,
        expected: u64,
    ) -> Result<(), MapError> {
        let field = format!("{layer} data");
        let attributes = self.attributes(element, &field, &["encoding", "compression"])?;
        let (encoding, compression) = (attributes.get("encoding"), attributes.get("compression"));
        if encoding != Some("csv") || compression.is_some() {
            return Err(self.error(
                field,
                MapErrorKind::UnsupportedEncoding {
                    encoding: encoding.map(str::to_owned),
                    compression: compression.map(str::to_owned),
                },
            ));
        }
        let mut text = None;
        if !empty {
            loop {
                match self.event(&field)? {
                    Event::Text(content) if text.is_none() => {
                        let content = content.unescape().map_err(|e| self.xml_error(&field, e))?;
                        text = Some(content.into_owned());
                    }
                    Event::End(_) => break,
                    Event::Eof => {
                        return Err(self.error(
                            field,
                            MapErrorKind::Parse("the file ends inside this element".into()),
                        ));
                    }
                    other => {
                        return Err(self.error(
                            field,
                            MapErrorKind::Parse(format!(
                                "{} is not supported in layer data, which must be plain CSV text",
                                describe(&other)
                            )),
                        ));
                    }
                }
            }
        }
        let found = u64::try_from(text.unwrap_or_default().split(',').count()).unwrap_or(u64::MAX);
        if found != expected {
            return Err(self.error(field, MapErrorKind::TileCountMismatch { expected, found }));
        }
        Ok(())
    }
}

fn utf8<'b>(file: &Path, bytes: &'b [u8]) -> Result<&'b str, MapError> {
    let error = |reason: String| MapError {
        file: file.to_owned(),
        field: "file".into(),
        kind: MapErrorKind::Parse(reason),
    };
    // `quick-xml` skips a byte order mark silently, which shifts its reported positions; the
    // mark is rejected first so that it is named as the cause (M1 review 2, P2-A item 1).
    if bytes.starts_with(b"\xEF\xBB\xBF") {
        return Err(error(
            "a UTF-8 byte order mark at the start of the file is not supported".into(),
        ));
    }
    std::str::from_utf8(bytes).map_err(|e| error(format!("the file is not UTF-8 text: {e}")))
}

/// The external files that `bevy_ecs_tiled` 0.13.4 preloads for a map or tileset, found by its
/// own line-based text scan (`tiled/reader.rs`, `extract_external_paths`), reproduced exactly:
/// on each line, the first `source="` and the first `template="`, up to the next `"`, if the
/// value ends in `.tsx` or `.tx`.
fn renderer_external_references(text: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for line in text.lines() {
        for marker in ["source=\"", "template=\""] {
            if let Some(start) = line.find(marker) {
                let rest = &line[start + marker.len()..];
                if let Some(end) = rest.find('"') {
                    let path = &rest[..end];
                    if path.ends_with(".tsx") || path.ends_with(".tx") {
                        paths.push(path.to_owned());
                    }
                }
            }
        }
    }
    paths
}

/// The renderer must find exactly the external files the check accepted, or it would load a
/// tileset the simulation never checked, or miss one it did (M1 review 1, P1-1).
fn check_external_references(file: &Path, text: &str, expected: &[String]) -> Result<(), MapError> {
    let found = renderer_external_references(text);
    if found != expected {
        return Err(MapError {
            file: file.to_owned(),
            field: "external file references".into(),
            kind: MapErrorKind::ExternalReferenceMismatch {
                found,
                expected: expected.to_vec(),
            },
        });
    }
    Ok(())
}

/// A file name in the directory of the referencing file: ASCII letters, digits, `_`, `-`, and
/// `.`, not starting with `.`, ending in `extension`. No `/`, `\`, or `..` can occur, so the
/// file stays inside the map's directory (and Bevy's approved asset root), and none of the
/// characters Bevy's asset paths give a meaning to (`#`, `:`) can occur.
fn is_plain_file_name(name: &str, extension: &str) -> bool {
    name.len() > extension.len() && name.ends_with(extension) && is_plain_name(name)
}

fn plain_file_name_rule(extension: &str) -> String {
    format!(
        "a file name in the map's directory, of ASCII letters, digits, '_', '-', and '.', not \
         starting with '.', ending in {extension:?}"
    )
}

fn is_plain_name(name: &str) -> bool {
    (1..=255).contains(&name.len())
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}

/// XML whitespace (space, tab, carriage return, line feed).
fn is_whitespace(text: &BytesText<'_>) -> bool {
    text.iter()
        .all(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
}

/// The value of attribute `name`, for naming an element in an error before its attributes have
/// been checked; empty if absent or malformed.
fn label(element: &BytesStart<'_>, name: &str) -> String {
    element
        .try_get_attribute(name)
        .ok()
        .flatten()
        .and_then(|attribute| {
            attribute
                .unescape_value()
                .ok()
                .map(|value| value.into_owned())
        })
        .unwrap_or_default()
}

fn element_name(element: &BytesStart<'_>) -> String {
    String::from_utf8_lossy(element.name().as_ref()).into_owned()
}

fn describe(event: &Event<'_>) -> &'static str {
    match event {
        Event::Start(_) | Event::Empty(_) => "an element",
        Event::End(_) => "an end tag",
        Event::Text(_) => "text",
        Event::CData(_) => "a CDATA section",
        Event::Comment(_) => "a comment",
        Event::Decl(_) => "an XML declaration",
        Event::PI(_) => "a processing instruction",
        Event::DocType(_) => "a document type declaration",
        Event::Eof => "the end of the file",
    }
}
