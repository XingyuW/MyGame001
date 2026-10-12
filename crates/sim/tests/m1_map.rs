//! M1 tests: loading maps into the `Grid` (PLAN.md section 7, M1; invariant I8) and the grid's
//! part in `state_hash` (invariant I3).
//!
//! Committed fixtures live in `tests/fixtures/maps/`. Further malformed variants are derived from
//! the valid fixture by exact text edits and written to Cargo's temporary directory for
//! integration tests, each under a name unique to its test.

use std::path::{Path, PathBuf};

use bevy_app::App;
use sim::{
    Cell, CellPos, Grid, LoadedMap, MAX_MAP_SIDE, MapErrorKind, SimPlugin, SimSchedule, load_map,
    state_hash,
};

const GRASS: Cell = Cell {
    water: false,
    passable: true,
    buildable: true,
    ore: 0,
};
const ORE: Cell = Cell {
    water: false,
    passable: true,
    buildable: false,
    ore: 750,
};
const ROCK: Cell = Cell {
    water: false,
    passable: false,
    buildable: false,
    ore: 0,
};

/// The cells of `valid_4x3.tmx`, indexed `[y][x]`.
const VALID_CELLS: [[Cell; 4]; 3] = [
    [GRASS, GRASS, ORE, ROCK],
    [GRASS, ORE, ORE, GRASS],
    [ROCK, GRASS, GRASS, GRASS],
];

const VALID_MAP_SIZE: &str = r#"width="4" height="3" tilewidth"#;
const VALID_LAYER_HEADER: &str = r#"<layer id="1" name="terrain" width="4" height="3">"#;
const VALID_DATA: &str = "1,1,2,3,\n1,2,2,1,\n3,1,1,1\n";
const MAP_END: &str = "</map>";

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/maps")
        .join(name)
}

fn valid_text() -> String {
    std::fs::read_to_string(fixture("valid_4x3.tmx")).expect("valid fixture exists")
}

/// Writes `contents` to the temporary file `name` and returns its path.
fn temp_file(name: &str, contents: &[u8]) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("m1_map");
    std::fs::create_dir_all(&dir).expect("temporary directory");
    let path = dir.join(name);
    std::fs::write(&path, contents).expect("temporary file");
    path
}

/// The valid fixture with each `(from, to)` edit applied; `from` must occur exactly once.
fn variant(name: &str, edits: &[(&str, &str)]) -> PathBuf {
    let mut text = valid_text();
    for (from, to) in edits {
        assert_eq!(text.matches(from).count(), 1, "fixture edit {from:?}");
        text = text.replacen(from, to, 1);
    }
    temp_file(&format!("{name}.tmx"), text.as_bytes())
}

/// A valid map of the given size whose cell (x, y) holds the tile with global id `gid(x, y)`.
fn sized_map(name: &str, width: u32, height: u32, gid: impl Fn(u32, u32) -> u32) -> PathBuf {
    let rows: Vec<String> = (0..height)
        .map(|y| {
            (0..width)
                .map(|x| gid(x, y).to_string())
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect();
    let data = format!("{}\n", rows.join(",\n"));
    variant(
        name,
        &[
            (
                VALID_MAP_SIZE,
                &format!(r#"width="{width}" height="{height}" tilewidth"#),
            ),
            (
                VALID_LAYER_HEADER,
                &format!(r#"<layer id="1" name="terrain" width="{width}" height="{height}">"#),
            ),
            (VALID_DATA, &data),
        ],
    )
}

fn load_ok(path: &Path) -> LoadedMap {
    load_map(path).unwrap_or_else(|error| panic!("{error}"))
}

/// Asserts that loading `path` fails with `kind` at `field`, and that the message names the
/// file, the field, and the reason.
fn assert_rejected(path: &Path, field: &str, kind: MapErrorKind) {
    let error = load_map(path).expect_err("the map must be rejected");
    assert_eq!(
        (error.field.as_str(), &error.kind),
        (field, &kind),
        "{error}"
    );
    assert_eq!(error.file, path);
    let message = error.to_string();
    for part in [
        path.display().to_string(),
        field.to_string(),
        kind.to_string(),
    ] {
        assert!(message.contains(&part), "{message:?} lacks {part:?}");
    }
}

fn assert_grid_matches(grid: &Grid, width: u32, height: u32, cell: impl Fn(u32, u32) -> Cell) {
    assert_eq!((grid.width(), grid.height()), (width, height));
    for y in 0..height {
        for x in 0..width {
            let pos = CellPos::new(x as i32, y as i32);
            assert_eq!(grid.cell(pos), Some(&cell(x, y)), "cell ({x}, {y})");
        }
    }
}

// --- Positive -------------------------------------------------------------------------------

#[test]
fn valid_fixture_loads_every_cell_in_tiled_coordinates() {
    let map = load_ok(&fixture("valid_4x3.tmx"));
    assert_eq!((map.tile_width, map.tile_height), (64, 32));
    assert_grid_matches(&map.grid, 4, 3, |x, y| VALID_CELLS[y as usize][x as usize]);
}

#[test]
fn loading_is_deterministic() {
    assert_eq!(
        load_ok(&fixture("valid_4x3.tmx")),
        load_ok(&fixture("valid_4x3.tmx"))
    );
}

#[test]
fn single_cell_and_thin_maps_load() {
    // Tile ids 1 to 3 (gid) cycle along the map so that a transposed reading would be detected.
    let gid = |x: u32, y: u32| (x + 2 * y) % 3 + 1;
    let cell = |x: u32, y: u32| [GRASS, ORE, ROCK][((x + 2 * y) % 3) as usize];
    for (width, height) in [(1, 1), (1, 7), (7, 1)] {
        let path = sized_map(&format!("thin_{width}x{height}"), width, height, gid);
        assert_grid_matches(&load_ok(&path).grid, width, height, cell);
    }
}

#[test]
fn external_tileset_loads() {
    let text = valid_text();
    let start = text.find("<tileset").expect("embedded tileset");
    let end = text.find("</tileset>").expect("embedded tileset end") + "</tileset>".len();
    let tileset = text[start..end].replacen(r#" firstgid="1""#, "", 1);
    temp_file(
        "external.tsx",
        format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{tileset}\n").as_bytes(),
    );
    let map = text.replacen(
        &text[start..end],
        r#"<tileset firstgid="1" source="external.tsx"/>"#,
        1,
    );
    let path = temp_file("external_tileset.tmx", map.as_bytes());
    assert_grid_matches(&load_ok(&path).grid, 4, 3, |x, y| {
        VALID_CELLS[y as usize][x as usize]
    });
}

// --- Boundary -------------------------------------------------------------------------------

#[test]
fn grid_contains_exactly_the_map_cells() {
    let grid = load_ok(&fixture("valid_4x3.tmx")).grid;
    for (x, y) in [(0, 0), (3, 0), (0, 2), (3, 2)] {
        assert!(grid.contains(CellPos::new(x, y)), "({x}, {y})");
    }
    for (x, y) in [
        (-1, 0),
        (0, -1),
        (4, 0),
        (0, 3),
        (4, 3),
        (-1, -1),
        (i32::MIN, 0),
        (0, i32::MAX),
    ] {
        let pos = CellPos::new(x, y);
        assert!(!grid.contains(pos), "({x}, {y})");
        assert_eq!(grid.cell(pos), None, "({x}, {y})");
    }
}

#[test]
fn empty_grid_contains_no_cell() {
    let grid = Grid::default();
    assert_eq!((grid.width(), grid.height()), (0, 0));
    assert!(!grid.contains(CellPos::new(0, 0)));
}

#[test]
fn maximum_map_size_loads() {
    let side = MAX_MAP_SIDE;
    for (width, height) in [(side, side), (side, 1), (1, side)] {
        let path = sized_map(&format!("max_{width}x{height}"), width, height, |_, _| 1);
        assert_grid_matches(&load_ok(&path).grid, width, height, |_, _| GRASS);
    }
}

#[test]
fn map_one_cell_beyond_the_maximum_is_rejected() {
    let over = MAX_MAP_SIDE + 1;
    for (width, height) in [(over, 1), (1, over)] {
        let path = sized_map(&format!("over_{width}x{height}"), width, height, |_, _| 1);
        assert_rejected(
            &path,
            r#"map attributes "width" and "height""#,
            MapErrorKind::TooLarge { width, height },
        );
    }
}

#[test]
fn ore_amount_limits() {
    let with_ore = |name: &str, value: &str| {
        variant(
            name,
            &[(
                r#"<property name="ore" type="int" value="750"/>"#,
                &format!(r#"<property name="ore" type="int" value="{value}"/>"#),
            )],
        )
    };
    let largest = load_ok(&with_ore("ore_max", &i32::MAX.to_string())).grid;
    assert_eq!(
        largest.cell(CellPos::new(2, 0)).map(|c| c.ore),
        Some(i32::MAX as u32)
    );
    let zero = load_ok(&with_ore("ore_zero", "0")).grid;
    assert_eq!(zero.cell(CellPos::new(2, 0)).map(|c| c.ore), Some(0));
    assert_rejected(
        &with_ore("ore_negative", "-1"),
        r#"tileset "fixture" tile 1 property "ore""#,
        MapErrorKind::NegativeOre(-1),
    );
}

#[test]
fn the_optional_look_property_is_presentation_data_per_cell() {
    // Without the property every cell's look is 0, and the terrain is unaffected by it.
    let plain = load_ok(&fixture("valid_4x3.tmx"));
    assert_eq!(plain.looks, vec![0; 12]);
    let with_look = |name: &str, property: &str| {
        variant(
            name,
            &[(
                r#"<property name="ore" type="int" value="750"/>"#,
                &format!(r#"<property name="ore" type="int" value="750"/>{property}"#),
            )],
        )
    };
    let looked = load_ok(&with_look(
        "look_ok",
        r#"<property name="look" type="int" value="255"/>"#,
    ));
    assert_eq!(looked.grid, plain.grid);
    // Tile 1 (gid 2) is the ore tile: cells (2, 0), (1, 1) and (2, 1).
    let expected: Vec<u8> = VALID_DATA
        .split(',')
        .map(|gid| if gid.trim() == "2" { 255 } else { 0 })
        .collect();
    assert_eq!(looked.looks, expected);
    for (name, value) in [("look_negative", "-1"), ("look_large", "256")] {
        assert_rejected(
            &with_look(
                name,
                &format!(r#"<property name="look" type="int" value="{value}"/>"#),
            ),
            r#"tileset "fixture" tile 1 property "look""#,
            MapErrorKind::InvalidValue {
                found: value.into(),
                expected: "an integer from 0 to 255".into(),
            },
        );
    }
    assert_rejected(
        &with_look(
            "look_string",
            r#"<property name="look" type="string" value="forest"/>"#,
        ),
        r#"tileset "fixture" tile 1 property "look""#,
        MapErrorKind::WrongPropertyType {
            expected: "int",
            found: "string",
        },
    );
}

// --- Negative: the cases named by PLAN.md section 7, M1 ---------------------------------------

#[test]
fn missing_layer_is_rejected() {
    assert_rejected(
        &fixture("missing_layer.tmx"),
        r#"layer "terrain""#,
        MapErrorKind::MissingLayer,
    );
}

#[test]
fn unknown_tile_property_is_rejected() {
    assert_rejected(
        &fixture("unknown_property.tmx"),
        r#"tileset "fixture" tile 0 property "height""#,
        MapErrorKind::UnknownProperty,
    );
}

#[test]
fn zero_size_is_rejected() {
    assert_rejected(
        &fixture("zero_size.tmx"),
        r#"map attributes "width" and "height""#,
        MapErrorKind::ZeroSize {
            width: 0,
            height: 3,
        },
    );
    let path = variant(
        "zero_height",
        &[
            (VALID_MAP_SIZE, r#"width="4" height="0" tilewidth"#),
            (
                VALID_LAYER_HEADER,
                r#"<layer id="1" name="terrain" width="4" height="0">"#,
            ),
            (VALID_DATA, ""),
        ],
    );
    assert_rejected(
        &path,
        r#"map attributes "width" and "height""#,
        MapErrorKind::ZeroSize {
            width: 4,
            height: 0,
        },
    );
}

// --- Negative: map header -------------------------------------------------------------------

#[test]
fn non_isometric_orientations_are_rejected() {
    // `bevy_ecs_tiled` panics on staggered and oblique maps (docs/DECISIONS.md, A2), so these
    // must never get past the loader.
    for orientation in ["orthogonal", "staggered", "hexagonal", "oblique"] {
        let path = variant(
            &format!("orientation_{orientation}"),
            &[(
                r#"orientation="isometric""#,
                &format!(r#"orientation="{orientation}""#),
            )],
        );
        assert_rejected(
            &path,
            r#"map attribute "orientation""#,
            MapErrorKind::UnsupportedOrientation(orientation.into()),
        );
    }
}

#[test]
fn infinite_map_is_rejected() {
    let path = variant("infinite", &[(r#"infinite="0""#, r#"infinite="1""#)]);
    assert_rejected(&path, r#"map attribute "infinite""#, MapErrorKind::Infinite);
}

#[test]
fn zero_tile_size_is_rejected() {
    let path = variant(
        "zero_tile_size",
        &[(
            r#"tilewidth="64" tileheight="32" infinite"#,
            r#"tilewidth="0" tileheight="32" infinite"#,
        )],
    );
    assert_rejected(
        &path,
        r#"map attributes "tilewidth" and "tileheight""#,
        MapErrorKind::ZeroTileSize {
            width: 0,
            height: 32,
        },
    );
}

// --- Negative: properties -------------------------------------------------------------------

const SCHEMA_PROPERTY: &str = r#"<property name="schema_version" type="int" value="2"/>"#;

#[test]
fn schema_version_is_required_and_checked() {
    let missing = variant("schema_missing", &[(SCHEMA_PROPERTY, "")]);
    assert_rejected(
        &missing,
        r#"map property "schema_version""#,
        MapErrorKind::MissingProperty,
    );
    let newer = variant(
        "schema_newer",
        &[(
            SCHEMA_PROPERTY,
            r#"<property name="schema_version" type="int" value="3"/>"#,
        )],
    );
    assert_rejected(
        &newer,
        r#"map property "schema_version""#,
        MapErrorKind::UnsupportedSchemaVersion(3),
    );
    let text = variant(
        "schema_string",
        &[(
            SCHEMA_PROPERTY,
            r#"<property name="schema_version" value="1"/>"#,
        )],
    );
    assert_rejected(
        &text,
        r#"map property "schema_version""#,
        MapErrorKind::WrongPropertyType {
            expected: "int",
            found: "string",
        },
    );
}

#[test]
fn unknown_map_layer_and_tileset_properties_are_rejected() {
    let extra = r#"<property name="author" value="x"/>"#;
    let on_map = variant(
        "unknown_map_property",
        &[(SCHEMA_PROPERTY, &format!("{SCHEMA_PROPERTY}{extra}"))],
    );
    assert_rejected(
        &on_map,
        r#"map property "author""#,
        MapErrorKind::UnknownProperty,
    );

    let on_layer = variant(
        "unknown_layer_property",
        &[(
            VALID_LAYER_HEADER,
            &format!("{VALID_LAYER_HEADER}<properties>{extra}</properties>"),
        )],
    );
    assert_rejected(
        &on_layer,
        r#"layer "terrain" property "author""#,
        MapErrorKind::UnknownProperty,
    );

    let image = r#"<image source="fixture_tiles.png" width="192" height="32"/>"#;
    let on_tileset = variant(
        "unknown_tileset_property",
        &[(image, &format!("<properties>{extra}</properties>{image}"))],
    );
    assert_rejected(
        &on_tileset,
        r#"tileset "fixture" property "author""#,
        MapErrorKind::UnknownProperty,
    );
}

#[test]
fn missing_and_mistyped_tile_properties_are_rejected() {
    let missing = variant(
        "tile_property_missing",
        &[(r#"<property name="ore" type="int" value="750"/>"#, "")],
    );
    assert_rejected(
        &missing,
        r#"tileset "fixture" tile 1 property "ore""#,
        MapErrorKind::MissingProperty,
    );

    let mistyped = variant(
        "tile_property_mistyped",
        &[(
            r#"<property name="ore" type="int" value="750"/>"#,
            r#"<property name="ore" type="float" value="750"/>"#,
        )],
    );
    assert_rejected(
        &mistyped,
        r#"tileset "fixture" tile 1 property "ore""#,
        MapErrorKind::WrongPropertyType {
            expected: "int",
            found: "float",
        },
    );

    let flag = variant(
        "tile_flag_mistyped",
        &[(
            r#"<property name="passable" type="bool" value="false"/>"#,
            r#"<property name="passable" value="no"/>"#,
        )],
    );
    assert_rejected(
        &flag,
        r#"tileset "fixture" tile 2 property "passable""#,
        MapErrorKind::WrongPropertyType {
            expected: "bool",
            found: "string",
        },
    );
}

// --- Negative: tilesets, layers, and cells --------------------------------------------------

#[test]
fn tileset_without_single_image_is_rejected() {
    let path = variant(
        "tileset_without_image",
        &[(
            r#"<image source="fixture_tiles.png" width="192" height="32"/>"#,
            "",
        )],
    );
    assert_rejected(
        &path,
        r#"tileset "fixture""#,
        MapErrorKind::TilesetWithoutImage,
    );
}

#[test]
fn layers_other_than_one_terrain_tile_layer_are_rejected() {
    let renamed = variant(
        "layer_renamed",
        &[(
            VALID_LAYER_HEADER,
            r#"<layer id="1" name="ground" width="4" height="3">"#,
        )],
    );
    assert_rejected(&renamed, r#"layer "ground""#, MapErrorKind::UnexpectedLayer);

    let second_layer = format!(
        "<layer id=\"2\" name=\"terrain\" width=\"4\" height=\"3\"><data encoding=\"csv\">{VALID_DATA}</data></layer>{MAP_END}"
    );
    let duplicate = variant("layer_duplicate", &[(MAP_END, &second_layer)]);
    assert_rejected(
        &duplicate,
        r#"layer "terrain""#,
        MapErrorKind::UnexpectedLayer,
    );

    let objects = variant(
        "layer_objects",
        &[(
            MAP_END,
            &format!(r#"<objectgroup id="2" name="units"/>{MAP_END}"#),
        )],
    );
    assert_rejected(&objects, r#"layer "units""#, MapErrorKind::UnexpectedLayer);
}

#[test]
fn layer_size_differing_from_map_is_rejected() {
    let path = variant(
        "layer_size_mismatch",
        &[
            (
                VALID_LAYER_HEADER,
                r#"<layer id="1" name="terrain" width="3" height="4">"#,
            ),
            (VALID_DATA, "1,1,1,\n1,1,1,\n1,1,1,\n1,1,1\n"),
        ],
    );
    assert_rejected(
        &path,
        r#"layer "terrain""#,
        MapErrorKind::LayerSizeMismatch {
            width: 3,
            height: 4,
        },
    );
}

#[test]
fn layer_data_with_wrong_entry_count_is_rejected_without_panic() {
    // Regression guard: `tiled` 0.16.0 panics when a short layer is accessed (see `precheck`).
    for (name, data, found) in [
        ("data_short", "1,1,2,3,\n1,2,2,1,\n3,1,1\n", 11),
        ("data_long", "1,1,2,3,\n1,2,2,1,\n3,1,1,1,1\n", 13),
        ("data_empty", "", 1),
    ] {
        let path = variant(name, &[(VALID_DATA, data)]);
        assert_rejected(
            &path,
            r#"layer "terrain" data"#,
            MapErrorKind::TileCountMismatch {
                expected: 12,
                found,
            },
        );
    }
}

#[test]
fn non_csv_layer_data_is_rejected() {
    let base64 = variant(
        "data_base64",
        &[
            (r#"<data encoding="csv">"#, r#"<data encoding="base64">"#),
            (VALID_DATA, "AQAAAA=="),
        ],
    );
    assert_rejected(
        &base64,
        r#"layer "terrain" data"#,
        MapErrorKind::UnsupportedEncoding {
            encoding: Some("base64".into()),
            compression: None,
        },
    );
    let compressed = variant(
        "data_compressed",
        &[(
            r#"<data encoding="csv">"#,
            r#"<data encoding="csv" compression="zlib">"#,
        )],
    );
    assert_rejected(
        &compressed,
        r#"layer "terrain" data"#,
        MapErrorKind::UnsupportedEncoding {
            encoding: Some("csv".into()),
            compression: Some("zlib".into()),
        },
    );
}

#[test]
fn empty_cell_is_rejected() {
    let path = variant(
        "empty_cell",
        &[(VALID_DATA, "1,1,2,3,\n1,2,0,1,\n3,1,1,1\n")],
    );
    assert_rejected(
        &path,
        r#"layer "terrain" cell (2, 1)"#,
        MapErrorKind::EmptyCell,
    );
}

#[test]
fn undefined_tile_is_rejected() {
    // Global id 4 is tile 3 of the tileset, which has no definition and so no properties.
    let path = variant(
        "undefined_tile",
        &[(VALID_DATA, "1,1,2,3,\n1,2,2,1,\n3,1,1,4\n")],
    );
    assert_rejected(
        &path,
        r#"layer "terrain" cell (3, 2)"#,
        MapErrorKind::UndefinedTile {
            tileset: "fixture".into(),
            id: 3,
        },
    );
}

// --- Negative: unreadable and malformed files -----------------------------------------------

#[test]
fn missing_file_is_a_read_error() {
    let path = fixture("does_not_exist.tmx");
    let error = load_map(&path).expect_err("missing file");
    assert_eq!(error.file, path);
    assert_eq!(error.field, "file");
    assert!(matches!(error.kind, MapErrorKind::Read(_)), "{error}");
}

#[test]
fn malformed_xml_and_wrong_root_are_parse_errors() {
    let not_xml = temp_file("not_xml.tmx", b"this is not a map");
    let wrong_root = temp_file(
        "wrong_root.tmx",
        br#"<?xml version="1.0"?><tileset name="x" tilewidth="64" tileheight="32"/>"#,
    );
    let missing_external = variant(
        "missing_external_tileset",
        &[(
            r#"<tileset firstgid="1" name="fixture""#,
            r#"<tileset firstgid="1" source="nowhere.tsx"/><tileset firstgid="9" name="fixture""#,
        )],
    );
    for path in [not_xml, wrong_root] {
        let error = load_map(&path).expect_err("malformed file");
        assert_eq!(error.field, "file", "{error}");
        assert!(matches!(error.kind, MapErrorKind::Parse(_)), "{error}");
    }
    // M1 review 2, P2-A item 3: a missing external tileset was reported as a parse error of the
    // field "file"; the fault is the reference's `source` attribute, and the reason is that the
    // named file cannot be read. This case asserts that exactly.
    let error = load_map(&missing_external).expect_err("missing external tileset");
    assert_eq!(error.file, missing_external, "{error}");
    assert_eq!(
        error.field, r#"tileset "nowhere.tsx" attribute "source""#,
        "{error}"
    );
    let tsx = missing_external.with_file_name("nowhere.tsx");
    assert!(
        matches!(&error.kind, MapErrorKind::Read(reason) if reason.contains(&tsx.display().to_string())),
        "{error}"
    );
}

#[test]
fn truncated_map_files_never_panic() {
    // Invariant I8: every prefix of a valid map is rejected with an error, never a panic, and
    // never yields a partial map. The XML parser accepts an unterminated final end tag at the
    // end of the file (`</map` without `>`); such a prefix holds the complete map and loads it.
    let text = valid_text();
    let full = load_ok(&fixture("valid_4x3.tmx"));
    let closing_tag = text.find(MAP_END).expect("closing tag");
    let complete = closing_tag + MAP_END.len();
    for len in 0..text.len() {
        let path = temp_file("truncated.tmx", &text.as_bytes()[..len]);
        match load_map(&path) {
            Ok(map) => {
                assert!(len > closing_tag, "prefix of {len} bytes loaded");
                assert_eq!(map, full, "prefix of {len} bytes");
            }
            Err(error) => assert!(len < complete, "prefix of {len} bytes: {error}"),
        }
    }
}

// --- Integration: the grid inside the simulation --------------------------------------------

fn sim_app_with_map(seed: u64, map: &LoadedMap) -> App {
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed });
    // Match setup: the map is installed before the first tick.
    app.insert_resource(map.grid.clone());
    app
}

fn hash_trace(app: &mut App, ticks: u64) -> Vec<u64> {
    let mut trace = vec![state_hash(app.world_mut())];
    for _ in 0..ticks {
        app.world_mut().run_schedule(SimSchedule);
        trace.push(state_hash(app.world_mut()));
    }
    trace
}

#[test]
fn sim_plugin_starts_with_an_empty_grid() {
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 0 });
    assert_eq!(*app.world().resource::<Grid>(), Grid::default());
}

#[test]
fn installed_grid_is_unchanged_by_ticks() {
    let map = load_ok(&fixture("valid_4x3.tmx"));
    let mut app = sim_app_with_map(1, &map);
    for _ in 0..100 {
        app.world_mut().run_schedule(SimSchedule);
    }
    assert_eq!(*app.world().resource::<Grid>(), map.grid);
}

#[test]
fn same_map_and_seed_give_equal_hashes_at_every_tick() {
    let map = load_ok(&fixture("valid_4x3.tmx"));
    let trace_a = hash_trace(&mut sim_app_with_map(4, &map), 200);
    let trace_b = hash_trace(&mut sim_app_with_map(4, &map), 200);
    assert_eq!(trace_a, trace_b);
}

#[test]
fn hash_covers_every_cell_attribute() {
    // Maps that differ in one attribute of one cell must hash differently at every tick.
    let base = load_ok(&fixture("valid_4x3.tmx"));
    let edits: [(&str, &str, &str); 4] = [
        ("hash_ore", r#"value="750""#, r#"value="751""#),
        (
            "hash_passable",
            r#"<property name="passable" type="bool" value="false"/>"#,
            r#"<property name="passable" type="bool" value="true"/>"#,
        ),
        (
            "hash_buildable",
            r#"<property name="buildable" type="bool" value="true"/>"#,
            r#"<property name="buildable" type="bool" value="false"/>"#,
        ),
        ("hash_tile", VALID_DATA, "1,1,2,3,\n1,2,2,1,\n3,1,1,2\n"),
    ];
    let base_trace = hash_trace(&mut sim_app_with_map(4, &base), 20);
    for (name, from, to) in edits {
        let other = load_ok(&variant(name, &[(from, to)]));
        assert_ne!(other.grid, base.grid, "{name}");
        let trace = hash_trace(&mut sim_app_with_map(4, &other), 20);
        for (tick, (a, b)) in base_trace.iter().zip(&trace).enumerate() {
            assert_ne!(a, b, "{name}: hashes collide at tick {tick}");
        }
    }
}
